#![allow(unreachable_pub)] // triggers false positives because we have both a binary and library

use clap::Parser;
use cot_cli::args::{
    Cli, CliCommands, Commands, HELP_LONG_FLAG, HELP_SHORT_FLAG, MigrationCommands,
    PACKAGE_LONG_FLAG, PACKAGE_SHORT_FLAG, RELEASE_FLAG, extract_package_arg,
};
use cot_cli::{handlers, project};
use tracing_subscriber::util::SubscriberInitExt;

fn is_top_level_help(args: &[String]) -> bool {
    if !args
        .iter()
        .any(|a| a == HELP_LONG_FLAG || a == HELP_SHORT_FLAG)
    {
        return false;
    }

    let mut rest = args.iter().skip(1).peekable();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            HELP_LONG_FLAG | HELP_SHORT_FLAG | RELEASE_FLAG => {}
            PACKAGE_SHORT_FLAG | PACKAGE_LONG_FLAG => {
                let Some(value) = rest.next() else {
                    return false;
                };
                if value.starts_with('-') {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

fn main() -> anyhow::Result<()> {
    let raw: Vec<String> = std::env::args().collect();
    let release = raw.iter().any(|a| a == RELEASE_FLAG);
    let package = extract_package_arg(&raw);

    if is_top_level_help(&raw) {
        let project = project::load(&std::env::current_dir()?, release, package.as_deref())?;
        handlers::handle_combined_help(project.as_ref())?;
        return Ok(());
    }

    let cli = Cli::parse();

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
        },
        Commands::External(args) => {
            let project = project::load(&std::env::current_dir()?, release, package.as_deref())?;
            handlers::handle_external(&args, project, release)
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
    fn top_level_help_accepts_only_global_flags() {
        assert!(is_top_level_help(&args(&["cot", "--help"])));
        assert!(is_top_level_help(&args(&["cot", "-h"])));
        assert!(is_top_level_help(&args(&[
            "cot",
            "--release",
            "-p",
            "blog",
            "--help"
        ])));
        assert!(is_top_level_help(&args(&[
            "cot",
            "--package",
            "blog",
            "-h",
            "--release"
        ])));
    }

    #[test]
    fn top_level_help_rejects_subcommands_and_non_help_invocations() {
        assert!(!is_top_level_help(&args(&["cot"])));
        assert!(!is_top_level_help(&args(&["cot", "migration", "--help"])));
        assert!(!is_top_level_help(&args(&["cot", "serve", "-h"])));
        assert!(!is_top_level_help(&args(&["cot", "--version"])));
    }

    #[test]
    fn top_level_help_treats_missing_package_value_as_not_top_level_help() {
        assert!(!is_top_level_help(&args(&["cot", "-p", "--help"])));
        assert!(!is_top_level_help(&args(&["cot", "--package", "-h"])));
    }
}
