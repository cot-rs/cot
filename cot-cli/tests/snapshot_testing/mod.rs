use std::process::Command;

pub use assert_cmd::prelude::*;
pub use insta_cmd::assert_cmd_snapshot;

pub use crate::cot_cli;

mod cli;
mod help;
mod migration;
mod new;

pub fn cot_clis_with_verbosity(cmd: Command) -> Vec<Command> {
    let get_cmd = |arg: &str| {
        let mut cmd_clone = Command::new(cmd.get_program());
        cmd_clone.args(cmd.get_args());
        if let Some(dir) = cmd.get_current_dir() {
            cmd_clone.current_dir(dir);
        }
        cmd_clone.arg(arg);
        cmd_clone
    };
    vec![
        get_cmd("-q"),
        get_cmd("-v"),
        get_cmd("-vv"),
        get_cmd("-vvv"),
        get_cmd("-vvvv"),
        get_cmd("-vvvvv"),
    ]
}

/// Build a `Command` for the `cot_cli` crate binary with variadic command-line
/// arguments.
///
/// The arguments can be anything that is allowed by `Command::arg`.
#[macro_export]
macro_rules! cot_cli {
    ( $( $arg:expr ),* ) => {
        {
            let mut cmd = crate::snapshot_testing::cot_cli_cmd();
            $(
                cmd.arg($arg);
            )*
            cmd
        }
    }
}

/// Get the command for the Cot CLI binary under test.
///
/// By default, this is the binary defined in this crate.
/// However, if the `COT_CLI_TEST_CMD` environment variable is set, its value is
/// used instead. Its value should be an absolute path to the desired
/// `cot-cli` program to test.
///
/// This environment variable makes it possible to run the test suite on
/// different versions of Cot CLI, such as a final release build or a
/// Docker image. For example:
///
///     COT_CLI_TEST_CMD="$PWD"/custom-cot-cli cargo test --test cli
pub fn cot_cli_cmd() -> Command {
    if let Ok(np) = std::env::var("COT_CLI_TEST_CMD") {
        Command::new(np)
    } else {
        Command::cargo_bin("cot").expect("cot-cli should be executable")
    }
}

pub fn get_logging_filters() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            r"/private/var/folders/([^/]+/)+?T/",
            r"/PRIVATE_MACOS_PATH/",
        ), // Redact macOS temp path
        (r"(?m)^.*?Z\[0m ", "TIMESTAMP"), // Remove timestamp
    ]
}
