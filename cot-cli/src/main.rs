#![allow(unreachable_pub)] // triggers false positives because we have both a binary and library

mod migration_generator;
mod new_project;
mod utils;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_verbosity_flag::Verbosity;
use tracing_subscriber::util::SubscriberInitExt;

use crate::migration_generator::{MigrationGeneratorOptions, list_migrations, make_migrations};
use crate::new_project::{CotSource, new_project};

#[derive(Debug, Parser)]
#[command(
    name = "cot",
    version,
    about,
    long_about = None
)]
struct Cli {
    #[command(flatten)]
    verbose: Verbosity,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Create a new Cot project
    New(ProjectNewArgs),

    /// Manage migrations for a Cot project
    #[command(subcommand)]
    Migration(MigrationCommands),

    /// Manage Cot CLI
    #[command(subcommand)]
    Cli(CliCommands),
}

#[derive(Debug, Args)]
struct ProjectNewArgs {
    /// Path to the directory to create the new project in
    path: PathBuf,
    /// Set the resulting crate name (defaults to the directory name)
    #[arg(long)]
    name: Option<String>,
    #[command(flatten)]
    source: CotSourceArgs,
}

#[derive(Debug, Subcommand)]
enum MigrationCommands {
    /// List all migrations for a Cot project
    List(MigrationListArgs),
    /// Generate migrations for a Cot project
    Make(MigrationMakeArgs),
}

#[derive(Debug, Args)]
struct MigrationListArgs {
    /// Path to the crate directory to list migrations for  [default: current
    /// directory]
    path: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct MigrationMakeArgs {
    /// Path to the crate directory to generate migrations for [default: current
    /// directory]
    path: Option<PathBuf>,
    /// Name of the app to use in the migration (default: crate name)
    #[arg(long)]
    app_name: Option<String>,
    /// Directory to write the migrations to (default: migrations/ directory
    /// in the crate's src/ directory)
    #[arg(long)]
    output_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
#[group(multiple = false)]
struct CotSourceArgs {
    /// Use the latest `cot` version from git instead of a published crate
    #[arg(long, group = "cot_source")]
    use_git: bool,
    /// Use `cot` from the specified path instead of a published crate
    #[arg(long, group = "cot_source")]
    cot_path: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum CliCommands {
    /// Generate manpages for the Cot CLI
    Manpages(ManpagesArgs),
    /// Generate completions for the Cot CLI
    Completions(CompletionsArgs),
}

#[derive(Debug, Args)]
struct ManpagesArgs {
    /// Directory to write the manpages to [default: current directory]
    #[arg(short, long)]
    output_dir: Option<PathBuf>,
    /// Create the directory if it doesn't exist
    #[arg(short, long)]
    create: bool,
}

#[derive(Debug, Args)]
struct CompletionsArgs {
    /// Shell to generate completions for
    shell: clap_complete::Shell,
}

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
        Commands::New(args) => handle_new_project(args),
        Commands::Cli(cmd) => match cmd {
            CliCommands::Manpages(args) => handle_cli_manpages(args),
            CliCommands::Completions(args) => handle_cli_completions(args),
        },
        Commands::Migration(cmd) => match cmd {
            MigrationCommands::List(args) => handle_migration_list(args),
            MigrationCommands::Make(args) => handle_migration_make(args),
        },
    }
}

fn handle_new_project(ProjectNewArgs { path, name, source }: ProjectNewArgs) -> anyhow::Result<()> {
    let project_name = match name {
        None => {
            let dir_name = path
                .file_name()
                .with_context(|| format!("file name not present: {}", path.display()))?;
            dir_name.to_string_lossy().into_owned()
        }
        Some(name) => name,
    };

    let cot_source = if source.use_git {
        CotSource::Git
    } else if let Some(path) = &source.cot_path {
        CotSource::Path(path)
    } else {
        CotSource::PublishedCrate
    };
    new_project(&path, &project_name, &cot_source).with_context(|| "unable to create project")
}

fn handle_migration_list(MigrationListArgs { path }: MigrationListArgs) -> anyhow::Result<()> {
    let path = path.unwrap_or(PathBuf::from("."));
    let migrations = list_migrations(&path).with_context(|| "unable to list migrations")?;
    for (app_name, migs) in migrations {
        for mig in migs {
            println!("{app_name}\t{mig}");
        }
    }

    Ok(())
}

fn handle_migration_make(
    MigrationMakeArgs {
        path,
        app_name,
        output_dir,
    }: MigrationMakeArgs,
) -> anyhow::Result<()> {
    let path = path.unwrap_or(PathBuf::from("."));
    let options = MigrationGeneratorOptions {
        app_name,
        output_dir,
    };
    make_migrations(&path, options).with_context(|| "unable to create migrations")
}

fn handle_cli_manpages(ManpagesArgs { output_dir, create }: ManpagesArgs) -> anyhow::Result<()> {
    let output_dir = output_dir.unwrap_or(PathBuf::from("."));
    if create {
        std::fs::create_dir_all(&output_dir).context("unable to create output directory")?;
    }
    clap_mangen::generate_to(Cli::command(), output_dir)
        .context("unable to generate manpages in output directory")
}

#[expect(clippy::unnecessary_wraps)]
fn handle_cli_completions(CompletionsArgs { shell }: CompletionsArgs) -> anyhow::Result<()> {
    generate_completions(shell, &mut std::io::stdout());

    Ok(())
}

fn generate_completions(shell: clap_complete::Shell, writer: &mut impl std::io::Write) {
    clap_complete::generate(shell, &mut Cli::command(), "cot", writer);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_project_wrong_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let args = ProjectNewArgs {
            path: temp_dir.path().to_path_buf(),
            name: None,
            source: CotSourceArgs {
                use_git: false,
                cot_path: None,
            },
        };

        let result = handle_new_project(args);

        assert!(result.is_err());
    }

    #[test]
    fn migration_list_wrong_directory() {
        let args = MigrationListArgs {
            path: Some(PathBuf::from("nonexistent")),
        };

        let result = handle_migration_list(args);

        assert!(result.is_err());
    }

    #[test]
    fn migration_make_wrong_directory() {
        let args = MigrationMakeArgs {
            path: Some(PathBuf::from("nonexistent")),
            app_name: None,
            output_dir: None,
        };

        let result = handle_migration_make(args);

        assert!(result.is_err());
    }

    #[test]
    fn generate_manpages() {
        let temp_dir = tempfile::tempdir().unwrap();
        let args = ManpagesArgs {
            output_dir: Some(temp_dir.path().to_path_buf()),
            create: true,
        };

        let result = handle_cli_manpages(args);

        assert!(result.is_ok());
        assert!(temp_dir.path().join("cot.1").exists());
    }

    #[test]
    fn generate_completions_shell() {
        let mut output = Vec::new();

        generate_completions(clap_complete::Shell::Bash, &mut output);

        assert!(!output.is_empty());
    }
}
