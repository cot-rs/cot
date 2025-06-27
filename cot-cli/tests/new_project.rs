use std::env;
use std::path::PathBuf;

use cot_cli::new_project::{CotSource, new_project};

#[test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: extern static `pidfd_spawnp` is not supported by Miri"
)]
fn new_project_compile_test() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_path = temp_dir.path().join("my_project");

    let cot_cli_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cot_workspace_path = cot_cli_path.parent().unwrap().join("cot");
    new_project(
        &project_path,
        "my_project",
        &CotSource::Path(&cot_workspace_path),
    )
    .unwrap();

    let output = cot_cli::test_utils::project_cargo(&project_path)
        .arg("run")
        .arg("--quiet")
        .arg("--")
        .arg("check")
        .output()
        .unwrap();

    let status = output.status;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(status.success(), "status: {status}, stderr: {stderr}");
    assert!(
        stdout.contains("Success verifying the configuration"),
        "status: {status}, stderr: {stderr}"
    );
}
