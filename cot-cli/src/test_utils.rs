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

pub const WORKSPACE_STUB: &str = "[workspace]\nresolver = \"3\"";

#[derive(Debug, Copy, Clone)]
enum CargoCommand {
    Init,
    New,
}

#[must_use]
pub fn get_nth_crate_name(i: u8) -> String {
    format!("cargo-test-crate-{i}")
}

pub fn make_workspace_package(path: &Path, packages: u8) -> anyhow::Result<()> {
    let workspace_cargo_toml = path.join("Cargo.toml");
    std::fs::write(workspace_cargo_toml, WORKSPACE_STUB)?;

    for i in 0..packages {
        let package_path = path.join(get_nth_crate_name(i + 1));
        make_package(&package_path)?;
    }

    Ok(())
}

pub fn make_package(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        create_cargo_project(path, CargoCommand::Init)
    } else {
        create_cargo_project(path, CargoCommand::New)
    }
}

fn create_cargo_project(path: &Path, cmd: CargoCommand) -> anyhow::Result<()> {
    let mut base = cargo();

    let cmd = match cmd {
        CargoCommand::Init => base.arg("init"),
        CargoCommand::New => base.arg("new"),
    };

    cmd.arg(path).output()?;

    Ok(())
}
