//! Functionality to locate, build (only if necessary), and query a Cot-compiled
//! binary.
mod build;
mod cache;
mod discovery;

use std::path::{Path, PathBuf};

use anyhow::bail;
use cot::metadata::ProjectMetadata;
use cot::utils::cli::{StatusType, print_status_msg};

use crate::project::discovery::ResolvedBinary;

const RELEASE_PROFILE: &str = "release";
const DEBUG_PROFILE: &str = "debug";

#[derive(Debug)]
pub struct ProjectBinary {
    pub path: PathBuf,
    pub metadata: Option<ProjectMetadata>,
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
    build: bool,
) -> anyhow::Result<Option<ProjectBinary>> {
    let Some(resolved) = discovery::resolve(path, release, package)? else {
        return Ok(None);
    };

    let ResolvedBinary {
        binary_path,
        project_dir,
        package_name,
        binary_name,
    } = resolved;

    if !binary_path.exists() {
        if !build {
            return Ok(None);
        }

        build::build_binary(&package_name, &binary_name, release)?;
        if !binary_path.exists() {
            bail!(
                "`cargo build` succeeded but `{}` still wasn't found at the expected path, \
                 this may mean the binary name `cot` resolved doesn't match what cargo built.",
                binary_path.display(),
            );
        }
    }

    // Guard against the `cot` CLI resolving to itself. This can happen when
    // running from within the `cot-cli` package or a workspace package whose
    // binary is the current executable. Querying it for `--metadata` would
    // either recurse or fail: only cot application binaries implement that
    // flag, not the CLI proxy.
    if discovery::is_current_executable(&binary_path) {
        return Ok(None);
    }

    let cache_path = cache::command_cache_path(&project_dir);
    let metadata = match cache::load_or_refresh(&binary_path, &cache_path) {
        Ok(meta) => meta,
        Err(e) => {
            print_status_msg(
                StatusType::Warning,
                &format!(
                    "could not determine `{}`'s cli commands, so they won't be \
                 listed when you run `cot --help`: {e:#}",
                    binary_path.display(),
                ),
            );
            None
        }
    };

    Ok(Some(ProjectBinary {
        path: binary_path,
        metadata,
    }))
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use cot::metadata::CommandMeta;
    use tempfile::TempDir;

    use super::*;
    use crate::project::cache::command_cache_path;

    pub(crate) fn canonical_temp_dir() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let tmp_path = temp_dir.path().canonicalize().unwrap();
        (temp_dir, tmp_path)
    }

    pub(crate) fn write_package_manifest(package_dir: &Path, package_name: &str, extra: &str) {
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

        if !extra.contains("[[bin]]") {
            let src_dir = package_dir.join("src");
            fs::create_dir_all(&src_dir).unwrap();
            fs::write(src_dir.join("main.rs"), "fn main() {}\n").unwrap();
        }
    }

    pub(crate) fn write_workspace_manifest(workspace_dir: &Path, members: &[&str]) {
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
            args: vec![],
        }
    }

    pub(crate) fn metadata(binary_name: &str, command_names: &[&str]) -> ProjectMetadata {
        ProjectMetadata {
            version: cot::metadata::METADATA_SCHEMA_VERSION,
            binary_name: binary_name.to_string(),
            commands: command_names.iter().map(|name| command(name)).collect(),
        }
    }

    #[cfg(unix)]
    pub(crate) fn write_metadata_script(path: &Path, metadata: &ProjectMetadata) {
        let json = serde_json::to_string(metadata).unwrap();
        write_shell_script(path, &format!("printf '%s\\n' '{json}'\n"));
    }

    #[cfg(unix)]
    pub(crate) fn write_shell_script(path: &Path, body: &str) {
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
        let (_guard, temp_dir) = canonical_temp_dir();

        let result = load(&temp_dir, false, None, true).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn load_errors_when_start_path_does_not_exist() {
        let (_guard, temp_dir) = canonical_temp_dir();

        let result = load(&temp_dir.join("missing"), false, None, true);

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
        let (_guard, temp_dir) = canonical_temp_dir();

        write_package_manifest(&temp_dir, "demo", "");

        let result = load(&temp_dir, false, None, false).unwrap();

        assert!(result.is_none());
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn load_reads_debug_binary_metadata_and_writes_cache() {
        let (_guard, temp_dir) = canonical_temp_dir();

        write_package_manifest(&temp_dir, "demo", "");
        let binary_path = temp_dir.join("target/debug/demo");
        write_metadata_script(&binary_path, &metadata("demo", &["serve"]));

        let project = load(&temp_dir, false, None, true).unwrap().unwrap();

        assert_eq!(project.path, binary_path);
        assert!(project.metadata.is_some());

        let metadata = project.metadata.unwrap();

        assert_eq!(metadata.binary_name, "demo");
        assert_eq!(metadata.commands[0].name, "serve");
        assert!(command_cache_path(&temp_dir).exists());
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn load_uses_release_profile_when_requested() {
        let (_guard, temp_dir) = canonical_temp_dir();

        write_package_manifest(&temp_dir, "demo", "");
        let binary_path = temp_dir.join("target/release/demo");
        write_metadata_script(&binary_path, &metadata("demo", &["serve"]));

        let project = load(&temp_dir, true, None, true).unwrap().unwrap();

        assert_eq!(project.path, binary_path);
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn load_uses_single_named_bin_target() {
        let (_guard, temp_dir) = canonical_temp_dir();

        write_package_manifest(
            &temp_dir,
            "demo",
            r#"[[bin]]
name = "server"
path = "src/server.rs"
"#,
        );
        let binary_path = temp_dir.join("target/debug/server");
        write_metadata_script(&binary_path, &metadata("server", &["serve"]));

        let project = load(&temp_dir, false, None, true).unwrap().unwrap();

        assert_eq!(project.path, binary_path);
        assert!(project.metadata.is_some());
        assert_eq!(project.metadata.unwrap().binary_name, "server");
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn load_uses_metadata_binary_override_before_bin_targets() {
        let (_guard, temp_dir) = canonical_temp_dir();

        write_package_manifest(
            &temp_dir,
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
        let binary_path = temp_dir.join("target/debug/api");
        write_metadata_script(&binary_path, &metadata("api", &["serve"]));

        let project = load(&temp_dir, false, None, true).unwrap().unwrap();

        assert_eq!(project.path, binary_path);
        assert!(project.metadata.is_some());
        assert_eq!(project.metadata.unwrap().binary_name, "api");
    }

    #[test]
    fn load_errors_on_multiple_bin_targets_without_override() {
        let (_guard, temp_dir) = canonical_temp_dir();

        write_package_manifest(
            &temp_dir,
            "demo",
            r#"[[bin]]
name = "api"
path = "src/api.rs"

[[bin]]
name = "worker"
path = "src/worker.rs"
"#,
        );

        let result = load(&temp_dir, false, None, true);

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("multiple [[bin]] targets"));
        assert!(message.contains("[package.metadata.cot]"));
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn load_falls_back_to_no_metadata_on_command_failure() {
        let (_guard, temp_dir) = canonical_temp_dir();
        write_package_manifest(&temp_dir, "demo", "");
        let binary_path = temp_dir.join("target/debug/demo");
        write_shell_script(
            &binary_path,
            "echo stdout message\necho stderr message >&2\nexit 42\n",
        );

        let project = load(&temp_dir, false, None, true).unwrap().unwrap();

        assert!(project.metadata.is_none());
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn load_falls_back_to_no_metadata_on_invalid_json() {
        let (_guard, temp_dir) = canonical_temp_dir();
        write_package_manifest(&temp_dir, "demo", "");
        let binary_path = temp_dir.join("target/debug/demo");
        write_shell_script(&binary_path, "echo 'not json'\n");

        let project = load(&temp_dir, false, None, true).unwrap().unwrap();

        assert!(project.metadata.is_none());
    }

    #[test]
    fn workspace_root_requires_package_when_ambiguous() {
        let (_guard, temp_dir) = canonical_temp_dir();
        write_workspace_manifest(&temp_dir, &["api", "web"]);
        write_package_manifest(&temp_dir.join("api"), "api", "");
        write_package_manifest(&temp_dir.join("web"), "web", "");

        let result = load(&temp_dir, false, None, true);

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("multiple packages found"));
        assert!(message.contains("api"));
        assert!(message.contains("web"));
    }

    #[test]
    fn workspace_package_flag_must_match_member() {
        let (_guard, temp_dir) = canonical_temp_dir();
        write_workspace_manifest(&temp_dir, &["api", "web"]);
        write_package_manifest(&temp_dir.join("api"), "api", "");
        write_package_manifest(&temp_dir.join("web"), "web", "");

        let result = load(&temp_dir, false, Some("missing"), true);

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("package `missing` not found"));
        assert!(message.contains("api"));
        assert!(message.contains("web"));
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn workspace_root_uses_selected_package_and_workspace_target_dir() {
        let (_guard, temp_dir) = canonical_temp_dir();
        write_workspace_manifest(&temp_dir, &["api", "web"]);
        write_package_manifest(&temp_dir.join("api"), "api", "");
        write_package_manifest(&temp_dir.join("web"), "web", "");
        let binary_path = temp_dir.join("target/debug/api");
        write_metadata_script(&binary_path, &metadata("api", &["check"]));

        let project = load(&temp_dir, false, Some("api"), true).unwrap().unwrap();

        assert_eq!(project.path, binary_path);
        assert!(&temp_dir.join("api").exists());
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn workspace_member_directory_uses_current_package_without_flag() {
        let (_guard, temp_dir) = canonical_temp_dir();
        write_workspace_manifest(&temp_dir, &["api", "web"]);
        write_package_manifest(&temp_dir.join("api"), "api", "");
        write_package_manifest(&temp_dir.join("web"), "web", "");
        let binary_path = temp_dir.join("target/debug/web");
        write_metadata_script(&binary_path, &metadata("web", &["check"]));

        let project = load(&temp_dir.join("web"), false, None, true)
            .unwrap()
            .unwrap();

        assert_eq!(project.path, binary_path);
        assert!(project.metadata.is_some());
        assert_eq!(project.metadata.unwrap().binary_name, "web");
    }
}
