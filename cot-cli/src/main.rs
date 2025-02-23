#![allow(unreachable_pub)] // triggers false positives because we have both a binary and library

mod migration_generator;
mod new_project;
mod utils;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use clap_verbosity_flag::Verbosity;
use tracing_subscriber::util::SubscriberInitExt;

use crate::migration_generator::{make_migrations, MigrationGeneratorOptions};
use crate::new_project::{new_project, CotSource};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(flatten)]
    verbose: Verbosity,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Create a new Cot project
    New {
        /// Path to the directory to create the new project in
        path: PathBuf,
        /// Set the resulting crate name (defaults to the directory name)
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        source: CotSourceArgs,
    },
    /// Generate migrations for a Cot project
    MakeMigrations {
        /// Path to the crate directory to generate migrations for (default:
        /// current directory)
        path: Option<PathBuf>,
        /// Name of the app to use in the migration (default: crate name)
        #[arg(long)]
        app_name: Option<String>,
        /// Directory to write the migrations to (default: migrations/ directory
        /// in the crate's src/ directory)
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
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
        Commands::New {
            path,
            name,
            source: cot_source,
        } => {
            let project_name = match name {
                None => {
                    let dir_name = path
                        .file_name()
                        .with_context(|| format!("file name not present: {}", path.display()))?;
                    dir_name.to_string_lossy().into_owned()
                }
                Some(name) => name,
            };

            let cot_source = if cot_source.use_git {
                CotSource::Git
            } else if let Some(path) = &cot_source.cot_path {
                CotSource::Path(path)
            } else {
                CotSource::PublishedCrate
            };
            new_project(&path, &project_name, &cot_source)
                .with_context(|| "unable to create project")?;
        }
        Commands::MakeMigrations {
            path,
            app_name,
            output_dir,
        } => {
            let path = path.unwrap_or_else(|| PathBuf::from("."));
            let options = MigrationGeneratorOptions {
                app_name,
                output_dir,
            };
            make_migrations(&path, options).with_context(|| "unable to create migrations")?;
        }
    }

    Ok(())
}
