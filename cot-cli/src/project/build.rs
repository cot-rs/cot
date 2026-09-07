//! Contains functionality to build a target project's binary.

use anyhow::Context;
use cot::utils::cli::{StatusType, print_status_msg};

use crate::args::{BINARY_FLAG, PACKAGE_SHORT_FLAG, RELEASE_FLAG};

const CARGO_ENV: &str = "CARGO";
pub(crate) fn build_binary(
    package_name: &str,
    binary_name: &str,
    release: bool,
) -> anyhow::Result<()> {
    print_status_msg(
        StatusType::Notice,
        &format!("no existing binary found for `{binary_name}`, building it now"),
    );

    let mut cmd = std::process::Command::new(
        std::env::var_os(CARGO_ENV).unwrap_or_else(|| "cargo".into()),
    );
    cmd.args([
        "build",
        PACKAGE_SHORT_FLAG,
        package_name,
        BINARY_FLAG,
        binary_name,
    ]);
    if release {
        cmd.arg(RELEASE_FLAG);
    }

    let status = cmd.status().context("failed to spawn `cargo build`")?;

    anyhow::ensure!(
        status.success(),
        "`cargo build` failed for `{package_name}`"
    );
    Ok(())
}
