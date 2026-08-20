//! Contains functionality to manage the caching mechanism of running
//! cot-compiled binaries. Metadata information is retrieved from the binary and
//! stored in a cache file located in the `.cot` directory in the root dir of
//! the project. When the cache is stale or unavailable, we query the binary and
//! populate the cache.

use std::fmt::Write;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::SystemTime;

use anyhow::{Context, bail};
use cot::metadata::{METADATA_FLAG, ProjectMetadata};
use cot::utils::cli::{StatusType, print_status_msg};
use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

const METADATA_TIMEOUT: core::time::Duration = core::time::Duration::from_secs(5);
const COT_DIR_NAME: &str = ".cot";
const CACHE_FILE_NAME: &str = "command-cache.json";
#[derive(Serialize, Deserialize)]
pub(crate) struct Cache {
    binary_mtime_secs: u64,
    metadata: ProjectMetadata,
}

pub(crate) fn command_cache_path(project_dir: &Path) -> PathBuf {
    project_dir.join(COT_DIR_NAME).join(CACHE_FILE_NAME)
}

pub(crate) fn load_or_refresh(
    binary_path: &Path,
    cache_path: &Path,
) -> anyhow::Result<Option<ProjectMetadata>> {
    let current_mtime_secs = mtime_secs(binary_path)?;

    // Fast path if we hit the cache
    if let Ok(bytes) = std::fs::read(cache_path)
        && let Ok(cache) = serde_json::from_slice::<Cache>(&bytes)
        && cache.binary_mtime_secs == current_mtime_secs
    {
        return Ok(Some(cache.metadata));
    }

    // slow path
    // stdout/stderr are piped and drained on separate threads to avoid a
    // deadlock. Pipe buffers are OS-bounded, so if the child fills one
    // while we're blocked waiting to timeout in `wait_timeout` or reading the other
    // output, its write blocks and deadlocks.
    // https://doc.rust-lang.org/std/process/index.html#handling-io
    // https://docs.rs/os_pipe/latest/os_pipe/#common-deadlocks-related-to-pipes
    let mut child = std::process::Command::new(binary_path)
        .arg(METADATA_FLAG)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn {}", binary_path.display()))?;

    let mut std_err_piped = child.stderr.take().expect("Stderr should be piped");
    let mut std_out_piped = child.stdout.take().expect("Stdout should be piped");

    let std_err_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        std_err_piped
            .read_to_end(&mut buf)
            .expect("reading to buffer should not fail");
        buf
    });

    let std_out_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        std_out_piped
            .read_to_end(&mut buf)
            .expect("reading to buffer should not fail");
        buf
    });

    let Some(status) = child
        .wait_timeout(METADATA_TIMEOUT)
        .with_context(|| format!("Failed to wait on {}", binary_path.display()))?
    else {
        let _ = child.kill();
        let _ = child.wait();
        bail!(
            "the `{}` binary did not respond within {:?} when queried for metadata.",
            binary_path.display(),
            METADATA_TIMEOUT
        );
    };

    let stdout = std_out_thread
        .join()
        .expect("joining thread handle should not fail");
    let stderr = std_err_thread
        .join()
        .expect("joining stderr thread should not fail");

    if !status.success() {
        let stderr_str = String::from_utf8_lossy(&stderr);

        // check for previous cot versions(<=0.7.0) without metadata support.
        let is_legacy_binary = status.code() == Some(2)
            && stderr_str.contains(&format!("unexpected argument '{METADATA_FLAG}'"));

        if is_legacy_binary {
            print_status_msg(
                StatusType::Warning,
                &format!(
                    "the `{}` binary doesn't recognize a flag `cot` uses to discover the binary's cli commands, \
                 so they won't be listed when you run `cot --help`. This usually means the binary \
                 was built against an older version of `cot`. To fix this, update your `cot`version",
                    binary_path.display(),
                ),
            );
            return Ok(None);
        }

        let mut msg = format!(
            "the `{}` binary exited unexpectedly while `cot` was trying to determine the binary's cli commands.",
            binary_path.display(),
        );
        if !stderr_str.trim().is_empty() {
            let _ = write!(msg, "\n\nstderr:\n{}", stderr_str.trim());
        }
        let stdout_str = String::from_utf8_lossy(&stdout);
        if !stdout_str.trim().is_empty() {
            let _ = write!(msg, "\n\nstdout:\n{}", stdout_str.trim());
        }
        bail!(msg);
    }

    if stdout.is_empty() {
        // The binary ran but the metadata flag was ignored
        bail!(
            "the `{}` binary produced no output for {METADATA_FLAG}",
            binary_path.display(),
        );
    }

    let metadata = parse_metadata(&stdout, binary_path)?;

    write_cache(
        cache_path,
        &Cache {
            binary_mtime_secs: current_mtime_secs,
            metadata: metadata.clone(),
        },
    )?;

    Ok(Some(metadata))
}

pub(crate) fn mtime_secs(path: &Path) -> anyhow::Result<u64> {
    let metadata = path.metadata()?;
    Ok(metadata
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs())
}

pub(crate) fn write_cache(cache_path: &Path, cache: &Cache) -> anyhow::Result<()> {
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
        ensure_cachedir_tag(parent)?;
    }
    std::fs::write(cache_path, serde_json::to_string(cache)?)?;
    Ok(())
}

const CACHEDIR_TAG_CONTENT: &str = "Signature: 8a477f597d28d172789f06886806bc55\n\
    # This file is a cache directory tag created by cot.\n\
    # For information about cache directory tags see https://bford.info/cachedir/\n";

fn ensure_cachedir_tag(cot_dir: &Path) -> anyhow::Result<()> {
    let tag_path = cot_dir.join("CACHEDIR.TAG");
    if !tag_path.exists() {
        std::fs::write(tag_path, CACHEDIR_TAG_CONTENT)?;
    }
    Ok(())
}

#[derive(Deserialize)]
struct MetadataVersionProbe {
    version: u32,
}

pub(crate) fn parse_metadata(bytes: &[u8], binary_path: &Path) -> anyhow::Result<ProjectMetadata> {
    // check the version first before attempting to deserialize so we can show a
    // clearer error message instead of the generic serde error message
    let probe: MetadataVersionProbe = serde_json::from_slice(bytes).with_context(|| {
        format!(
            "the `{}` binary returned metadata with no readable version field.",
            binary_path.display()
        )
    })?;

    anyhow::ensure!(
        probe.version == cot::metadata::METADATA_SCHEMA_VERSION,
        "the `{}` binary was built against a `cot` version with metadata schema v{}, \
         but this `cot-cli` expects v{}. Try updating cot-cli (`cargo install --locked cot-cli`) \
         or rebuilding the project.",
        binary_path.display(),
        probe.version,
        cot::metadata::METADATA_SCHEMA_VERSION,
    );

    serde_json::from_slice(bytes).with_context(|| {
        format!(
            "Binary `{}` returned invalid JSON for {METADATA_FLAG}\n\nstdout:\n{}",
            binary_path.display(),
            String::from_utf8_lossy(bytes).trim(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::load;
    use crate::project::tests::{
        canonical_temp_dir, metadata, write_metadata_script, write_package_manifest,
        write_shell_script,
    };

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn load_reuses_valid_cache_without_spawning_binary() {
        let (_guard, temp_dir) = canonical_temp_dir();
        write_package_manifest(&temp_dir, "demo", "");
        let binary_path = temp_dir.join("target/debug/demo");
        write_shell_script(
            &binary_path,
            "echo 'binary should not be queried' >&2\nexit 42\n",
        );
        let cache = Cache {
            binary_mtime_secs: mtime_secs(&binary_path).unwrap(),
            metadata: metadata("demo", &["cached"]),
        };
        write_cache(&command_cache_path(&temp_dir), &cache).unwrap();

        let project = load(&temp_dir, false, None, true).unwrap().unwrap();

        assert!(project.metadata.is_some());
        assert_eq!(project.metadata.unwrap().commands[0].name, "cached");
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn load_refreshes_stale_cache() {
        let (_guard, temp_dir) = canonical_temp_dir();
        write_package_manifest(&temp_dir, "demo", "");
        let binary_path = temp_dir.join("target/debug/demo");
        write_metadata_script(&binary_path, &metadata("demo", &["fresh"]));
        let cache = Cache {
            binary_mtime_secs: 0,
            metadata: metadata("demo", &["stale"]),
        };
        write_cache(&command_cache_path(&temp_dir), &cache).unwrap();

        let project = load(&temp_dir, false, None, true).unwrap().unwrap();

        assert!(project.metadata.is_some());
        assert_eq!(project.metadata.unwrap().commands[0].name, "fresh");
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn load_or_refresh_reports_command_failure_with_output() {
        let (_guard, temp_dir) = canonical_temp_dir();
        let binary_path = temp_dir.join("demo");
        write_shell_script(
            &binary_path,
            "echo stdout message\necho stderr message >&2\nexit 42\n",
        );
        let cache_path = command_cache_path(&temp_dir);

        let result = load_or_refresh(&binary_path, &cache_path);

        assert!(result.is_err());
        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains("exited unexpectedly"));
        assert!(message.contains("stdout message"));
        assert!(message.contains("stderr message"));
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn load_or_refresh_reports_invalid_json() {
        let (_guard, temp_dir) = canonical_temp_dir();
        let binary_path = temp_dir.join("demo");
        write_shell_script(&binary_path, "echo 'not json'\n");
        let cache_path = command_cache_path(&temp_dir);

        let result = load_or_refresh(&binary_path, &cache_path);

        assert!(result.is_err());
        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains("no readable version field"));
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    #[cfg(unix)]
    fn load_or_refresh_returns_none_for_legacy_binary() {
        let (_guard, temp_dir) = canonical_temp_dir();
        let binary_path = temp_dir.join("demo");
        write_shell_script(
            &binary_path,
            &format!("echo \"error: unexpected argument '{METADATA_FLAG}'\" >&2\nexit 2\n"),
        );
        let cache_path = command_cache_path(&temp_dir);

        let result = load_or_refresh(&binary_path, &cache_path).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn parse_metadata_reports_schema_version_mismatch() {
        let bytes = br#"{"version":999,"binary_name":"demo","commands":[]}"#;

        let result = parse_metadata(bytes, &PathBuf::from("target/debug/demo"));

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("metadata schema v999"));
        assert!(message.contains("cargo install --locked cot-cli"));
    }

    #[test]
    fn parse_metadata_succeeds_on_matching_shape() {
        let meta = metadata("demo", &["serve"]);
        let bytes = serde_json::to_vec(&meta).unwrap();

        let result = parse_metadata(&bytes, &PathBuf::from("target/debug/demo"));

        assert!(result.is_ok());
    }
}
