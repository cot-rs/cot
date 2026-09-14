#![allow(unreachable_pub)] // triggers false positives because we have both a binary and library

use std::ffi::OsString;

use clap::Parser;
use cot_cli::args::{
    BUILD_FLAG, Cli, CliCommands, Commands, HELP_LONG_FLAG, HELP_SHORT_FLAG, MigrationCommands,
    PACKAGE_LONG_FLAG, PACKAGE_SHORT_FLAG, RELEASE_FLAG, extract_package_arg,
};
use cot_cli::{handlers, project};
use tracing_subscriber::util::SubscriberInitExt;

fn resolve_help_request(args: &[String]) -> Option<Vec<String>> {
    if !args
        .iter()
        .any(|a| a == HELP_LONG_FLAG || a == HELP_SHORT_FLAG)
    {
        return None;
    }

    let mut path = Vec::new();
    let mut iter = args.iter().skip(1).peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            // short-circuit once we find a help flag
            HELP_LONG_FLAG | HELP_SHORT_FLAG => return Some(path),
            RELEASE_FLAG | BUILD_FLAG => {}
            PACKAGE_SHORT_FLAG | PACKAGE_LONG_FLAG => match iter.peek() {
                Some(v) if !v.starts_with('-') => {
                    iter.next();
                }
                _ => return None,
            },
            other if other.starts_with('-') => return None,
            other => path.push(other.to_string()),
        }
    }

    None
}

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

    if let Some(path) = resolve_help_request(cot_args) {
        let project = project::load(
            &std::env::current_dir()?,
            release,
            package.as_deref(),
            build,
        )?;
        handlers::handle_combined_help(project.as_ref(), &path)?;
        return Ok(());
    }

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
    fn top_level_help_returns_empty_path() {
        assert_eq!(
            resolve_help_request(&args(&["cot", "--help"])),
            Some(vec![])
        );
        assert_eq!(resolve_help_request(&args(&["cot", "-h"])), Some(vec![]));
    }

    #[test]
    fn top_level_help_accepts_global_flags_before_help() {
        assert_eq!(
            resolve_help_request(&args(&["cot", "--release", "-p", "blog", "--help"])),
            Some(vec![])
        );
        assert_eq!(
            resolve_help_request(&args(&["cot", "--package", "blog", "-h", "--release"])),
            Some(vec![])
        );
    }

    #[test]
    fn help_flag_short_circuits_ignoring_trailing_tokens() {
        assert_eq!(
            resolve_help_request(&args(&["cot", "--help", "foo"])),
            Some(vec![])
        );
        assert_eq!(
            resolve_help_request(&args(&["cot", "migration", "-h", "rollback"])),
            Some(vec!["migration".to_string()])
        );
    }

    #[test]
    fn subcommand_help_returns_path() {
        assert_eq!(
            resolve_help_request(&args(&["cot", "migration", "--help"])),
            Some(vec!["migration".to_string()])
        );
        assert_eq!(
            resolve_help_request(&args(&["cot", "migration", "rollback", "-h"])),
            Some(vec!["migration".to_string(), "rollback".to_string()])
        );
    }

    #[test]
    fn non_help_invocations_return_none() {
        assert_eq!(resolve_help_request(&args(&["cot"])), None);
        assert_eq!(resolve_help_request(&args(&["cot", "serve"])), None);
        assert_eq!(resolve_help_request(&args(&["cot", "--version"])), None);
    }

    #[test]
    fn missing_package_value_returns_none() {
        assert_eq!(resolve_help_request(&args(&["cot", "-p", "--help"])), None);
        assert_eq!(
            resolve_help_request(&args(&["cot", "--package", "-h"])),
            None
        );
    }

    #[test]
    fn unknown_flag_before_help_returns_none() {
        assert_eq!(
            resolve_help_request(&args(&["cot", "--unknown", "--help"])),
            None
        );
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
