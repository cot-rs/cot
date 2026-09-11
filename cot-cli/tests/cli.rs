use cot_cli::test_harness::CotProjectBuilder;
use tempfile::TempDir;

// It's pointless to run miri on UI tests
#[cfg(not(miri))]
mod snapshot_testing;

#[cfg(miri)]
mod snapshot_testing {
    pub fn cot_cli_path() -> std::path::PathBuf {
        unreachable!("miri tests are #[ignore]d")
    }
}

#[test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: socketpair: type 0x5 is unsupported"
)]
fn discovery_honors_cargo_target_dir_env_var() {
    let project = CotProjectBuilder::new(snapshot_testing::cot_cli_path())
        .build()
        .unwrap()
        .compile()
        .unwrap();

    let override_dir = TempDir::new().unwrap();
    project.bridge_binary_to(override_dir.path()).unwrap();

    let output = project
        .cot_cmd_raw(&["check"])
        .env("CARGO_TARGET_DIR", override_dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: socketpair: type 0x5 is unsupported"
)]
fn discovery_honors_project_level_cargo_config() {
    let project = CotProjectBuilder::new(snapshot_testing::cot_cli_path())
        .with_file(
            ".cargo/config.toml",
            "[build]\ntarget-dir = \"custom-target\"\n",
        )
        .build()
        .unwrap()
        .compile()
        .unwrap();

    let custom_target = project.path().join("custom-target");
    project.bridge_binary_to(&custom_target).unwrap();

    let output = project.cot_cmd_raw(&["check"]).output().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: socketpair: type 0x5 is unsupported"
)]
fn discovery_honors_global_cargo_config() {
    let project = CotProjectBuilder::new(snapshot_testing::cot_cli_path())
        .build()
        .unwrap()
        .compile()
        .unwrap();

    let fake_cargo_home = TempDir::new().unwrap();
    let custom_target = fake_cargo_home.path().join("shared-target");
    let normalized_target_path = custom_target.display().to_string().replace('\\', "/");
    std::fs::write(
        fake_cargo_home.path().join("config.toml"),
        format!("[build]\ntarget-dir = \"{normalized_target_path}\"\n"),
    )
    .unwrap();

    project.bridge_binary_to(&custom_target).unwrap();

    let output = project
        .cot_cmd_raw(&["check"])
        .env("CARGO_HOME", fake_cargo_home.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: socketpair: type 0x5 is unsupported"
)]
fn cargo_target_dir_env_wins_over_project_config() {
    let project = CotProjectBuilder::new(snapshot_testing::cot_cli_path())
        .with_file(
            ".cargo/config.toml",
            "[build]\ntarget-dir = \"from-config\"\n",
        )
        .build()
        .unwrap()
        .compile()
        .unwrap();

    let env_override = TempDir::new().unwrap();
    project.bridge_binary_to(env_override.path()).unwrap();

    let output = project
        .cot_cmd_raw(&["check"])
        .env("CARGO_TARGET_DIR", env_override.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
