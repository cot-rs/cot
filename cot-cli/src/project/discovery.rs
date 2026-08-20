//! Contains functionality to discover the cot-compiled binary path and its
//! target dir.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use cargo_metadata::{Metadata, MetadataCommand, Package, Target};

use crate::project::{DEBUG_PROFILE, RELEASE_PROFILE};

#[derive(Debug)]
pub(crate) struct ResolvedBinary {
    pub(crate) binary_path: PathBuf,
    pub(crate) project_dir: PathBuf,
    pub(crate) package_name: String,
    pub(crate) binary_name: String,
}

pub(crate) fn resolve(
    path: &Path,
    release: bool,
    package: Option<&str>,
) -> anyhow::Result<Option<ResolvedBinary>> {
    let Some(workspace_metadata) = load_cargo_metadata(path)? else {
        return Ok(None);
    };

    let resolved_package = resolve_package(&workspace_metadata, path, package)?;
    let binary_name = resolve_binary_name(resolved_package)?;
    let target_dir = workspace_metadata.target_directory.as_std_path();
    let profile = if release {
        RELEASE_PROFILE
    } else {
        DEBUG_PROFILE
    };

    #[cfg(target_os = "windows")]
    let binary_name = format!("{binary_name}.exe");

    let binary_path = target_dir.join(profile).join(&binary_name);

    let project_dir = resolved_package
        .manifest_path
        .parent()
        .context("package manifest path unexpectedly has no parent directory")?
        .as_std_path()
        .to_path_buf();

    Ok(Some(ResolvedBinary {
        binary_path,
        project_dir,
        package_name: resolved_package.name.to_string(),
        binary_name,
    }))
}

pub(crate) fn resolve_package<'a>(
    metadata: &'a Metadata,
    path: &Path,
    package: Option<&str>,
) -> anyhow::Result<&'a Package> {
    if let Some(name) = package {
        return metadata
            .packages
            .iter()
            .find(|p| p.name.as_str() == name)
            .with_context(|| {
                format!(
                    "package `{name}` not found in workspace.\nAvailable packages: {}",
                    available_packages(metadata)
                )
            });
    }

    if metadata.packages.len() == 1 {
        return Ok(&metadata.packages[0]);
    }

    current_package(metadata, path).ok_or_else(|| {
        anyhow::anyhow!(
            "multiple packages found in the workspace; specify which one to use with `-p <PACKAGE>`.\n\n\
             Available packages: {}",
            available_packages(metadata)
        )
    })
}

/// Resolve the binary name for a package:
///
/// 1. If the package has a `[package.metadata.cot.binary]` entry, use that.
/// 2. If the package has a single `[[bin]]` target, use that.
/// 3. If it has multiple, use `default-run` if set.
/// 4. Otherwise, error out and ask the user to disambiguate.
pub(crate) fn resolve_binary_name(package: &Package) -> anyhow::Result<String> {
    if let Some(name) = package
        .metadata
        .get("cot")
        .and_then(|c| c.get("binary"))
        .and_then(|b| b.as_str())
    {
        return Ok(name.to_string());
    }

    let bin_targets: Vec<&Target> = package.targets.iter().filter(|t| t.is_bin()).collect();

    match bin_targets.len() {
        0 => bail!(
            "package `{}` has no binary ([[bin]]) targets for `cot` to run.",
            package.name,
        ),
        1 => Ok(bin_targets[0].name.clone()),
        _ => {
            // if a default-run field exists lets use that
            // https://doc.rust-lang.org/cargo/reference/manifest.html#the-default-run-field
            if let Some(default_run) = &package.default_run {
                return Ok(default_run.clone());
            }

            bail!(
                "package `{}` has multiple [[bin]] targets.\n\
             Specify which one `cot` should use by adding to its Cargo.toml:\n\
             \n\
             [package.metadata.cot]\n\
             binary = \"your-binary-name\"",
                package.name,
            )
        }
    }
}

pub(crate) fn is_current_executable(binary_path: &Path) -> bool {
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

/// Runs `cargo metadata --no-deps` rooted at `path`.
///
/// `--no-deps` means this never touches the network or reads/writes
/// `Cargo.lock`: it only needs to parse the workspace's own manifests, so
/// it's safe to run on every `cot` invocation.
pub(crate) fn load_cargo_metadata(path: &Path) -> anyhow::Result<Option<Metadata>> {
    if !path.exists() {
        bail!("path does not exist: {}", path.display())
    }

    match MetadataCommand::new().no_deps().current_dir(path).exec() {
        Ok(metadata) => Ok(Some(metadata)),
        Err(cargo_metadata::Error::CargoMetadata { stderr })
            if stderr.contains("could not find `Cargo.toml`") =>
        {
            Ok(None)
        }
        Err(e) => Err(e).context("failed to run `cargo metadata`"),
    }
}

fn available_packages(metadata: &Metadata) -> String {
    metadata
        .packages
        .iter()
        .map(|p| p.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Finds the workspace member `path` is inside of, preferring the most
/// specific (deepest) match — mirrors how cargo resolves the "current
/// package" from the nearest enclosing manifest.
fn current_package<'a>(metadata: &'a Metadata, path: &Path) -> Option<&'a Package> {
    let path = path.canonicalize().ok()?;

    metadata
        .packages
        .iter()
        .filter(|pkg| {
            pkg.manifest_path
                .parent()
                .is_some_and(|dir| path.starts_with(dir.as_std_path()))
        })
        .max_by_key(|pkg| pkg.manifest_path.as_str().len())
}

#[cfg(test)]
mod tests {
    use super::*;

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
