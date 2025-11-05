use std::process::Command;

pub(crate) use insta_cmd::assert_cmd_snapshot;

pub(crate) use crate::cot_cli;

mod cli;
mod help;
mod migration;
mod new;

pub(crate) fn cot_clis_with_verbosity(cmd: &Command) -> Vec<Command> {
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
            let mut cmd = $crate::snapshot_testing::cot_cli_cmd();
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
pub(crate) fn cot_cli_cmd() -> Command {
    if let Ok(np) = std::env::var("COT_CLI_TEST_CMD") {
        Command::new(np)
    } else {
        Command::new(assert_cmd::cargo::cargo_bin!("cot"))
    }
}

const GENERIC_FILTERS: &[(&str, &str)] = &[
    (r"(?m)^.\[2m[\d-]+?T[\d:\.]+?Z.\[0m ", "TIMESTAMP "), // Remove timestamp
    (r"cot\.exe", r"cot"),                                 // Redact Windows .exe
];

const TEMP_PATH_FILTERS: &[(&str, &str)] = &[
    (r"(/private)?/var/folders/([^/]+/)+?T/", r"/tmp/"), // Redact macOS temp path
    (r"(C:)?\\.*\\Temp", "/tmp"),                        // Redact Windows temp path
    (r"\\{1,2}", "/"),                                   // Redact Windows path separators
];

const TEMP_PROJECT_FILTERS: &[(&str, &str)] = &[
    (r"cot-test-[^/]+", "TEMP_PATH"), // Remove temp dir path
];
