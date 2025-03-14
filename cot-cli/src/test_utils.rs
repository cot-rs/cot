use std::env;
use std::path::Path;
use std::process::Command;

fn raw_cargo() -> Command {
    match env::var_os("CARGO") {
        Some(cargo) => Command::new(cargo),
        None => Command::new("cargo"),
    }
}

#[must_use]
pub fn cargo() -> Command {
    let mut cmd = raw_cargo();
    cmd.env_remove("RUSTFLAGS");
    cmd.env("CARGO_INCREMENTAL", "0");

    cmd
}

#[must_use]
pub fn project_cargo(project_path: &Path) -> Command {
    let mut cmd = cargo();
    cmd.current_dir(project_path);
    cmd.env("CARGO_TARGET_DIR", project_path.join("target"));

    cmd
}
