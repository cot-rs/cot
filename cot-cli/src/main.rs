#![allow(unreachable_pub)] // triggers false positives because we have both a binary and library

use std::ffi::OsString;

use clap::Parser;
use cot_cli::args::{
    BUILD_FLAG, Cli, CliCommands, Commands, MigrationCommands, RELEASE_FLAG, extract_package_arg,
};
use cot_cli::{handlers, project};
use tracing_subscriber::util::SubscriberInitExt;

fn forwarded_args(
    clap_captured_args: &[OsString],
    args_after_double_dash: &[String],
) -> Vec<OsString> {
    clap_captured_args
        .iter()
        .cloned()
        .chain(args_after_double_dash.iter().map(OsString::from))
        .collect()
}

fn split_on_double_dash(raw: &[String]) -> (&[String], &[String]) {
    match raw.iter().position(|a| a == "--") {
        Some(i) => (&raw[..i], &raw[i + 1..]),
        None => (raw, &[]),
    }
}

fn main() -> anyhow::Result<()> {
    let raw: Vec<String> = std::env::args().collect();

    let (cot_args, forwarded_remaining_args) = split_on_double_dash(&raw);

    let release = cot_args.iter().any(|a| a == RELEASE_FLAG);
    let build = cot_args.iter().any(|b| b == BUILD_FLAG);
    let package = extract_package_arg(cot_args);

    let cli = Cli::parse_from(cot_args);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(cli.verbose.tracing_level_filter().into()),
        )
        .finish()
        .init();

    match cli.command {
        Commands::New(args) => handlers::handle_new_project(args),
        Commands::Cli(cmd) => match cmd {
            CliCommands::Manpages(args) => handlers::handle_cli_manpages(args),
            CliCommands::Completions(args) => handlers::handle_cli_completions(args),
        },
        Commands::Migration(cmd) => match cmd {
            MigrationCommands::List(args) => handlers::handle_migration_list(args),
            MigrationCommands::Make(args) => handlers::handle_migration_make(args),
            MigrationCommands::New(args) => handlers::handle_migration_new(args),
            MigrationCommands::External(args) => {
                let project = project::load(
                    &std::env::current_dir()?,
                    release,
                    package.as_deref(),
                    build,
                )?;
                let path = vec![
                    "migration".to_string(),
                    args[0].to_string_lossy().into_owned(),
                ];
                let remaining = forwarded_args(&args[1..], forwarded_remaining_args);
                handlers::handle_external(&path, &remaining, project, release)
            }
        },
        Commands::External(args) => {
            let project = project::load(
                &std::env::current_dir()?,
                release,
                package.as_deref(),
                build,
            )?;
            let path = vec![args[0].to_string_lossy().into_owned()];
            let remaining = forwarded_args(&args[1..], forwarded_remaining_args);
            handlers::handle_external(&path, &remaining, project, release)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|arg| (*arg).to_string()).collect()
    }

    #[test]
    fn forwarded_args_combines_captured_and_double_dash_tail() {
        let captured = vec![OsString::from("--dry-run")];
        let tail = vec!["--app".to_string(), "blog".to_string()];

        let result = forwarded_args(&captured, &tail);

        assert_eq!(
            result,
            vec![
                OsString::from("--dry-run"),
                OsString::from("--app"),
                OsString::from("blog"),
            ]
        );
    }

    #[test]
    fn forwarded_args_empty_inputs_produce_empty_vec() {
        assert!(forwarded_args(&[], &[]).is_empty());
    }

    #[test]
    fn split_on_double_dash_splits_at_delimiter() {
        let raw = args(&["cot", "check", "--", "--dry-run", "x"]);
        let (before, after) = split_on_double_dash(&raw);

        assert_eq!(before, &args(&["cot", "check"])[..]);
        assert_eq!(after, &args(&["--dry-run", "x"])[..]);
    }

    #[test]
    fn split_on_double_dash_without_delimiter_returns_all_before() {
        let raw = args(&["cot", "check"]);
        let (before, after) = split_on_double_dash(&raw);

        assert_eq!(before, &raw[..]);
        assert!(after.is_empty());
    }
}