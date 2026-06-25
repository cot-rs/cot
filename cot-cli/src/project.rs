use std::fmt::Write;
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

const COT_DIR_NAME: &str = ".cot";
const CACHE_FILE_NAME: &str = "command-cache.json";

fn command_cache_path(project_dir: &Path) -> PathBuf {
    project_dir.join(COT_DIR_NAME).join(CACHE_FILE_NAME)
}

#[derive(Debug)]
pub struct ProjectBinary {
    pub path: PathBuf,
    pub metadata: ProjectMetadata,
}

/// Find and load the project binary and its metadata.
///
/// `package` corresponds to `cot -p <PACKAGE> ...` or `--package <PACKAGE>`.
/// It's required when run from a workspace root
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

    // Guard against the `cot` CLI resolving to itself. This can happen when
    // running from within the `cot-cli` package or a workspace package whose
    // binary is the current executable. Querying it for `--metadata` would
    // either recurse or fail: only cot application binaries implement that
    // flag, not the CLI proxy.
    if is_current_executable(&binary_path) {
        return Ok(None);
    }

    let cache_path = command_cache_path(project_dir);
    let metadata = load_or_refresh_metadata(&binary_path, &cache_path).context(format!(
        "unable to load metadata from binary `{}`",
        binary_path.display()
    ))?;

    Ok(Some(ProjectBinary {
        path: binary_path,
        metadata,
    }))
}

fn is_current_executable(binary_path: &Path) -> bool {
    let Ok(current_exe) = std::env::current_exe() else {
        return false;
    };

    let Ok(binary_path) = binary_path.canonicalize() else {
        return false;
    };
    let Ok(current_exe) = current_exe.canonicalize() else {
        return false;
    };

    binary_path == current_exe
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
        "multiple packages found in the workspace; specify which one to use with `-p <PACKAGE>`.\n\n\
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
/// 1. If the package has a `[package.metadata.cot.binary]` entry (typically as
///    a result of disambiguating multiple binaries), use that.
/// 2. If the package has a single `[[bin]]` explicitly in `Cargo.toml`, use
///    that.
/// 3. Otherwise, use the package name.
fn resolve_binary_name(package_manager: &PackageManager) -> anyhow::Result<String> {
    let manifest: &Manifest = package_manager.get_manifest();

    if let Some(package) = &manifest.package
        && let Some(metadata) = &package.metadata
        && let Some(name) = metadata
            .get("cot")
            .and_then(|c| c.get("binary"))
            .and_then(|b| b.as_str())
    {
        return Ok(name.to_string());
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

    if let Ok(bytes) = std::fs::read(cache_path)
        && let Ok(cache) = serde_json::from_slice::<Cache>(&bytes)
        && cache.binary_mtime_secs == current_mtime_secs
    {
        return Ok(cache.metadata);
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
            let _ = write!(msg, "\n\nstderr:\n{}", stderr.trim());
        }

        if !stdout.trim().is_empty() {
            let _ = write!(msg, "\n\nstdout:\n{}", stdout.trim());
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

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use cot::metadata::CommandMeta;
    use tempfile::TempDir;

    use super::*;

    fn write_package_manifest(package_dir: &Path, package_name: &str, extra: &str) {
        fs::create_dir_all(package_dir).unwrap();
        fs::write(
            package_dir.join("Cargo.toml"),
            format!(
                r#"[package]
name = "{package_name}"
version = "0.1.0"
edition = "2024"

{extra}"#
            ),
        )
        .unwrap();
    }

    fn write_workspace_manifest(workspace_dir: &Path, members: &[&str]) {
        fs::write(
            workspace_dir.join("Cargo.toml"),
            format!(
                "[workspace]\nresolver = \"3\"\nmembers = [{}]\n",
                members
                    .iter()
                    .map(|member| format!("\"{member}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .unwrap();
    }

    fn command(name: &str) -> CommandMeta {
        CommandMeta {
            name: name.to_string(),
            about: None,
            aliases: vec![],
            subcommands: vec![],
        }
    }

    fn metadata(binary_name: &str, command_names: &[&str]) -> ProjectMetadata {
        ProjectMetadata {
            binary_name: binary_name.to_string(),
            commands: command_names.iter().map(|name| command(name)).collect(),
        }
    }

    #[cfg(unix)]
    fn write_metadata_script(path: &Path, metadata: &ProjectMetadata) {
        let json = serde_json::to_string(metadata).unwrap();
        write_shell_script(path, &format!("printf '%s\\n' '{json}'\n"));
    }

    #[cfg(unix)]
    fn write_shell_script(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, format!("#!/bin/sh\n{body}")).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn load_returns_none_without_cargo_manifest() {
        let temp_dir = TempDir::new().unwrap();

        let result = load(temp_dir.path(), false, None).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn load_errors_when_start_path_does_not_exist() {
        let temp_dir = TempDir::new().unwrap();

        let result = load(&temp_dir.path().join("missing"), false, None);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("path does not exist")
        );
    }

    #[test]
    fn load_returns_none_when_expected_binary_is_missing() {
        let temp_dir = TempDir::new().unwrap();
        write_package_manifest(temp_dir.path(), "demo", "");

        let result = load(temp_dir.path(), false, None).unwrap();

        assert!(result.is_none());
    }

    #[test]
    #[cfg(unix)]
    fn load_reads_debug_binary_metadata_and_writes_cache() {
        let temp_dir = TempDir::new().unwrap();
        write_package_manifest(temp_dir.path(), "demo", "");
        let binary_path = temp_dir.path().join("target/debug/demo");
        write_metadata_script(&binary_path, &metadata("demo", &["serve"]));

        let project = load(temp_dir.path(), false, None).unwrap().unwrap();

        assert_eq!(project.path, binary_path);
        assert_eq!(project.metadata.binary_name, "demo");
        assert_eq!(project.metadata.commands[0].name, "serve");
        assert!(command_cache_path(temp_dir.path()).exists());
    }

    #[test]
    #[cfg(unix)]
    fn load_uses_release_profile_when_requested() {
        let temp_dir = TempDir::new().unwrap();
        write_package_manifest(temp_dir.path(), "demo", "");
        let binary_path = temp_dir.path().join("target/release/demo");
        write_metadata_script(&binary_path, &metadata("demo", &["serve"]));

        let project = load(temp_dir.path(), true, None).unwrap().unwrap();

        assert_eq!(project.path, binary_path);
    }

    #[test]
    #[cfg(unix)]
    fn load_uses_single_named_bin_target() {
        let temp_dir = TempDir::new().unwrap();
        write_package_manifest(
            temp_dir.path(),
            "demo",
            r#"[[bin]]
name = "server"
path = "src/server.rs"
"#,
        );
        let binary_path = temp_dir.path().join("target/debug/server");
        write_metadata_script(&binary_path, &metadata("server", &["serve"]));

        let project = load(temp_dir.path(), false, None).unwrap().unwrap();

        assert_eq!(project.path, binary_path);
        assert_eq!(project.metadata.binary_name, "server");
    }

    #[test]
    #[cfg(unix)]
    fn load_uses_metadata_binary_override_before_bin_targets() {
        let temp_dir = TempDir::new().unwrap();
        write_package_manifest(
            temp_dir.path(),
            "demo",
            r#"[package.metadata.cot]
binary = "api"

[[bin]]
name = "api"
path = "src/api.rs"

[[bin]]
name = "worker"
path = "src/worker.rs"
"#,
        );
        let binary_path = temp_dir.path().join("target/debug/api");
        write_metadata_script(&binary_path, &metadata("api", &["serve"]));

        let project = load(temp_dir.path(), false, None).unwrap().unwrap();

        assert_eq!(project.path, binary_path);
        assert_eq!(project.metadata.binary_name, "api");
    }

    #[test]
    fn load_errors_on_multiple_bin_targets_without_override() {
        let temp_dir = TempDir::new().unwrap();
        write_package_manifest(
            temp_dir.path(),
            "demo",
            r#"[[bin]]
name = "api"
path = "src/api.rs"

[[bin]]
name = "worker"
path = "src/worker.rs"
"#,
        );

        let result = load(temp_dir.path(), false, None);

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("multiple [[bin]] targets"));
        assert!(message.contains("[package.metadata.cot]"));
    }

    #[test]
    fn workspace_root_requires_package_when_ambiguous() {
        let temp_dir = TempDir::new().unwrap();
        write_workspace_manifest(temp_dir.path(), &["api", "web"]);
        write_package_manifest(&temp_dir.path().join("api"), "api", "");
        write_package_manifest(&temp_dir.path().join("web"), "web", "");

        let result = load(temp_dir.path(), false, None);

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("multiple packages found"));
        assert!(message.contains("api"));
        assert!(message.contains("web"));
    }

    #[test]
    fn workspace_package_flag_must_match_member() {
        let temp_dir = TempDir::new().unwrap();
        write_workspace_manifest(temp_dir.path(), &["api", "web"]);
        write_package_manifest(&temp_dir.path().join("api"), "api", "");
        write_package_manifest(&temp_dir.path().join("web"), "web", "");

        let result = load(temp_dir.path(), false, Some("missing"));

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("package `missing` not found"));
        assert!(message.contains("api"));
        assert!(message.contains("web"));
    }

    #[test]
    #[cfg(unix)]
    fn workspace_root_uses_selected_package_and_workspace_target_dir() {
        let temp_dir = TempDir::new().unwrap();
        write_workspace_manifest(temp_dir.path(), &["api", "web"]);
        write_package_manifest(&temp_dir.path().join("api"), "api", "");
        write_package_manifest(&temp_dir.path().join("web"), "web", "");
        let binary_path = temp_dir.path().join("target/debug/api");
        write_metadata_script(&binary_path, &metadata("api", &["check"]));

        let project = load(temp_dir.path(), false, Some("api")).unwrap().unwrap();

        assert_eq!(project.path, binary_path);
        assert!(temp_dir.path().join("api").exists());
    }

    #[test]
    #[cfg(unix)]
    fn workspace_member_directory_uses_current_package_without_flag() {
        let temp_dir = TempDir::new().unwrap();
        write_workspace_manifest(temp_dir.path(), &["api", "web"]);
        write_package_manifest(&temp_dir.path().join("api"), "api", "");
        write_package_manifest(&temp_dir.path().join("web"), "web", "");
        let binary_path = temp_dir.path().join("target/debug/web");
        write_metadata_script(&binary_path, &metadata("web", &["check"]));

        let project = load(&temp_dir.path().join("web"), false, None)
            .unwrap()
            .unwrap();

        assert_eq!(project.path, binary_path);
        assert_eq!(project.metadata.binary_name, "web");
    }

    #[test]
    #[cfg(unix)]
    fn load_reuses_valid_cache_without_spawning_binary() {
        let temp_dir = TempDir::new().unwrap();
        write_package_manifest(temp_dir.path(), "demo", "");
        let binary_path = temp_dir.path().join("target/debug/demo");
        write_shell_script(
            &binary_path,
            "echo 'binary should not be queried' >&2\nexit 42\n",
        );
        let cache = Cache {
            binary_mtime_secs: mtime_secs(&binary_path).unwrap(),
            metadata: metadata("demo", &["cached"]),
        };
        write_cache(&command_cache_path(temp_dir.path()), &cache).unwrap();

        let project = load(temp_dir.path(), false, None).unwrap().unwrap();

        assert_eq!(project.metadata.commands[0].name, "cached");
    }

    #[test]
    #[cfg(unix)]
    fn load_refreshes_stale_cache() {
        let temp_dir = TempDir::new().unwrap();
        write_package_manifest(temp_dir.path(), "demo", "");
        let binary_path = temp_dir.path().join("target/debug/demo");
        write_metadata_script(&binary_path, &metadata("demo", &["fresh"]));
        let cache = Cache {
            binary_mtime_secs: 0,
            metadata: metadata("demo", &["stale"]),
        };
        write_cache(&command_cache_path(temp_dir.path()), &cache).unwrap();

        let project = load(temp_dir.path(), false, None).unwrap().unwrap();

        assert_eq!(project.metadata.commands[0].name, "fresh");
    }

    #[test]
    #[cfg(unix)]
    fn load_reports_metadata_command_failure_with_output() {
        let temp_dir = TempDir::new().unwrap();
        write_package_manifest(temp_dir.path(), "demo", "");
        let binary_path = temp_dir.path().join("target/debug/demo");
        write_shell_script(
            &binary_path,
            "echo stdout message\necho stderr message >&2\nexit 42\n",
        );

        let result = load(temp_dir.path(), false, None);

        assert!(result.is_err());
        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains("unable to load metadata"));
        assert!(message.contains("exited with status"));
        assert!(message.contains("stdout message"));
        assert!(message.contains("stderr message"));
    }

    #[test]
    #[cfg(unix)]
    fn load_reports_invalid_metadata_json() {
        let temp_dir = TempDir::new().unwrap();
        write_package_manifest(temp_dir.path(), "demo", "");
        let binary_path = temp_dir.path().join("target/debug/demo");
        write_shell_script(&binary_path, "echo 'not json'\n");

        let result = load(temp_dir.path(), false, None);

        assert!(result.is_err());
        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains(METADATA_FLAG));
        assert!(message.contains("not json"));
    }

    #[test]
    fn current_executable_matches_current_process() {
        let current_exe = std::env::current_exe().unwrap();

        assert!(is_current_executable(&current_exe));
    }

    #[test]
    fn current_executable_does_not_match_missing_path() {
        let missing = std::env::temp_dir().join("cot-cli-missing-test-binary");

        assert!(!is_current_executable(&missing));
    }
}
