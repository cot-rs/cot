#![allow(unreachable_pub)] // triggers false positives because we have both a binary and library

use clap::Parser;
use cot_cli::args::{Cli, CliCommands, Commands, MigrationCommands};
use cot_cli::handlers;
use tracing_subscriber::util::SubscriberInitExt;

fn main() -> anyhow::Result<()> {
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
        },
    }
}
