use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, bail};
use cargo_toml::Manifest;
use cot::metadata::{METADATA_FLAG, ProjectMetadata};
use serde::{Deserialize, Serialize};

use crate::utils::{CargoTomlManager, PackageManager, WorkspaceManager};

const RELEASE_PROFILE: &str = "release";
const DEBUG_PROFILE: &str = "debug";

#[derive(Serialize, Deserialize)]
struct Cache {
    binary_mtime_secs: u64,
    metadata: ProjectMetadata,
}

const CACHE_FILE_NAME: &str = ".command-cache.json";

pub struct ProjectBinary {
    pub path: PathBuf,
    pub metadata: ProjectMetadata,
}

/// Find and load the project binary and its metadata.
///
/// `package` corresponds to `cot -p <PACKAGE> ...` or `--package <PACKAGE>`,
/// mirroring `cargo`'s flag. It's required when run from a workspace root
/// (or any directory that doesn't unambiguously belong to one package) and
/// the workspace has more than one member.
pub fn load(
    path: &Path,
    release: bool,
    package: Option<&str>,
) -> anyhow::Result<Option<ProjectBinary>> {
    let Some(manager) = CargoTomlManager::from_path(path)? else {
        return Ok(None);
    };

    let (package_manager, target_dir_root): (&PackageManager, PathBuf) = match &manager {
        CargoTomlManager::Package(pm) => {
            let dir = pm.get_package_path().to_path_buf();
            (pm, dir)
        }
        CargoTomlManager::Workspace(wm) => {
            let pm = resolve_workspace_package(wm, package)?;
            (pm, wm.get_workspace_root().to_path_buf())
        }
    };

    let project_dir = package_manager.get_package_path();
    let binary_name = resolve_binary_name(package_manager)?;
    let target_dir = resolve_target_dir(&target_dir_root);
    let profile = if release {
        RELEASE_PROFILE
    } else {
        DEBUG_PROFILE
    };

    #[cfg(target_os = "windows")]
    let binary_name = format!("{binary_name}.exe");

    let binary_path = target_dir.join(profile).join(binary_name);

    if !binary_path.exists() {
        return Ok(None);
    }

    let cache_path = project_dir.join(CACHE_FILE_NAME);
    let metadata = load_or_refresh_metadata(&binary_path, &cache_path).context(format!(
        "unable to load metadata from binary `{}`",
        binary_path.display()
    ))?;

    Ok(Some(ProjectBinary {
        path: binary_path,
        metadata,
    }))
}

fn resolve_workspace_package<'a>(
    wm: &'a WorkspaceManager,
    package: Option<&str>,
) -> anyhow::Result<&'a PackageManager> {
    if let Some(name) = package {
        return wm.get_package_manager(name).with_context(|| {
            format!(
                "package `{name}` not found in workspace.\nAvailable packages: {}",
                available_packages(wm)
            )
        });
    }

    if let Some(pm) = wm.get_current_package_manager() {
        return Ok(pm);
    }

    bail!(
        "multiple packages found in the workspace; specify which one to use with `-p <PACKAGE>`.\n\
         Available packages: {}",
        available_packages(wm)
    )
}

fn available_packages(wm: &WorkspaceManager) -> String {
    wm.get_packages()
        .iter()
        .map(|p| p.get_package_name())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Resolve the binary name for a package:
///
/// 1. `[package.metadata.cot] binary = "..."` — explicit override, useful when
///    a crate has multiple `[[bin]]` targets
/// 2. A single `[[bin]]` entry — use its name
/// 3. Fall back to the package name (cargo's default when there's no explicit
///    `[[bin]]` and `src/main.rs` exists)
fn resolve_binary_name(package_manager: &PackageManager) -> anyhow::Result<String> {
    let manifest: &Manifest = package_manager.get_manifest();

    if let Some(package) = &manifest.package {
        if let Some(metadata) = &package.metadata {
            if let Some(name) = metadata
                .get("cot")
                .and_then(|c| c.get("binary"))
                .and_then(|b| b.as_str())
            {
                return Ok(name.to_string());
            }
        }
    }

    let named_bins: Vec<&str> = manifest
        .bin
        .iter()
        .filter_map(|b| b.name.as_deref())
        .collect();

    match named_bins.len() {
        0 => {}
        1 => return Ok(named_bins[0].to_string()),
        _ => bail!(
            "package `{}` has multiple [[bin]] targets.\n\
             Specify which one `cot` should use by adding to its Cargo.toml:\n\
             \n\
             [package.metadata.cot]\n\
             binary = \"your-binary-name\"",
            package_manager.get_package_name(),
        ),
    }

    manifest
        .package
        .as_ref()
        .map(|p| p.name.clone())
        .context("Cargo.toml has no [package] section and no [[bin]] targets")
}

fn resolve_target_dir(start_dir: &Path) -> PathBuf {
    let mut dir = start_dir;
    loop {
        let candidate = dir.join("target");
        if candidate.exists() {
            return candidate;
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
    start_dir.join("target")
}

fn load_or_refresh_metadata(
    binary_path: &Path,
    cache_path: &Path,
) -> anyhow::Result<ProjectMetadata> {
    let current_mtime_secs = mtime_secs(binary_path)?;

    if let Ok(bytes) = std::fs::read(cache_path) {
        if let Ok(cache) = serde_json::from_slice::<Cache>(&bytes) {
            if cache.binary_mtime_secs == current_mtime_secs {
                return Ok(cache.metadata);
            }
        }
    }

    let output = std::process::Command::new(binary_path)
        .arg(METADATA_FLAG)
        .output()
        .with_context(|| format!("Failed to spawn {}", binary_path.display()))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut msg = format!(
            "Binary `{}` exited with status {} when queried for metadata.",
            binary_path.display(),
            output.status,
        );

        if !stderr.trim().is_empty() {
            msg.push_str(&format!("\n\nstderr:\n{}", stderr.trim()));
        }

        if !stdout.trim().is_empty() {
            msg.push_str(&format!("\n\nstdout:\n{}", stdout.trim()));
        }
        bail!(msg);
    }

    let metadata: ProjectMetadata = serde_json::from_slice(&output.stdout).with_context(|| {
        let raw = String::from_utf8_lossy(&output.stdout);
        format!(
            "Binary `{}` returned invalid JSON for {METADATA_FLAG}.\n\nGot:\n{}",
            binary_path.display(),
            raw.trim(),
        )
    })?;

    write_cache(
        cache_path,
        &Cache {
            binary_mtime_secs: current_mtime_secs,
            metadata: metadata.clone(),
        },
    )?;

    Ok(metadata)
}

fn mtime_secs(path: &Path) -> anyhow::Result<u64> {
    let metadata = path.metadata()?;
    Ok(metadata
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs())
}

fn write_cache(cache_path: &Path, cache: &Cache) -> anyhow::Result<()> {
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(cache_path, serde_json::to_string(cache)?)?;
    Ok(())
}
