use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use clap_verbosity_flag::Verbosity;

pub const PACKAGE_LONG_FLAG: &str = "--package";
pub const PACKAGE_SHORT_FLAG: &str = "-p";
pub const RELEASE_FLAG: &str = "--release";
pub const HELP_LONG_FLAG: &str = "--help";
pub const HELP_SHORT_FLAG: &str = "-h";

#[derive(Debug, Parser)]
#[command(
    name = "cot",
    version,
    about,
    long_about = None
)]
pub struct Cli {
    /// Use target/release instead of target/debug when looking for the project
    /// binary
    #[arg(long, global = true)]
    release: bool,
    /// Package to use, in case you're running this in a workspace
    #[arg(short = 'p', long, global = true, value_name = "PACKAGE")]
    pub package: Option<String>,
    #[command(flatten)]
    pub verbose: Verbosity,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Create a new Cot project
    New(ProjectNewArgs),

    /// Manage migrations for a Cot project
    #[command(subcommand)]
    Migration(MigrationCommands),

    /// Manage Cot CLI
    #[command(subcommand)]
    Cli(CliCommands),

    #[command(external_subcommand)]
    External(Vec<OsString>),
}

#[derive(Debug, Args)]
pub struct ProjectNewArgs {
    /// Path to the directory to create the new project in
    pub path: PathBuf,
    /// Set the resulting crate name [default: the directory name]
    #[arg(long)]
    pub name: Option<String>,
    #[command(flatten)]
    pub source: CotSourceArgs,
}

#[derive(Debug, Subcommand)]
pub enum MigrationCommands {
    /// List all migrations for a Cot project
    List(MigrationListArgs),
    /// Generate migrations for a Cot project
    Make(MigrationMakeArgs),
    /// Create a new empty migration
    New(MigrationNewArgs),
}

#[derive(Debug, Args)]
pub struct MigrationNewArgs {
    /// Name of the migration
    pub name: String,
    /// Path to the crate directory to create the migration in [default: current
    /// directory]
    pub path: Option<PathBuf>,
    /// Name of the app to use in the migration (default: crate name)
    #[arg(long)]
    pub app_name: Option<String>,
}

#[derive(Debug, Args)]
pub struct MigrationListArgs {
    /// Path to the crate directory to list migrations for [default: current
    /// directory]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MigrationMakeArgs {
    /// Path to the crate directory to generate migrations for [default: current
    /// directory]
    pub path: Option<PathBuf>,
    /// Name of the app to use in the migration [default: crate name]
    #[arg(long)]
    pub app_name: Option<String>,
    /// Directory to write the migrations to [default: the migrations/ directory
    /// in the crate's src/ directory]
    #[arg(long)]
    pub output_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
#[group(multiple = false)]
pub struct CotSourceArgs {
    /// Use the latest `cot` version from git instead of a published crate
    #[arg(long, group = "cot_source")]
    pub use_git: bool,
    /// Use `cot` from the specified path instead of a published crate
    #[arg(long, group = "cot_source")]
    pub cot_path: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum CliCommands {
    /// Generate manpages for the Cot CLI
    Manpages(ManpagesArgs),
    /// Generate completions for the Cot CLI
    Completions(CompletionsArgs),
}

#[derive(Debug, Args)]
pub struct ManpagesArgs {
    /// Directory to write the manpages to [default: current directory]
    #[arg(short, long)]
    pub output_dir: Option<PathBuf>,
    /// Create the directory if it doesn't exist
    #[arg(short, long)]
    pub create: bool,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    pub shell: clap_complete::Shell,
}

/// Pulls `-p <name>` / `--package <name>` / `--package=<name>` out of raw
/// argv, before clap has parsed anything. Needed because `project::load`
/// must run before `Cli::parse()` for the `--help` interception path.
#[must_use]
pub fn extract_package_arg(raw: &[String]) -> Option<String> {
    let mut iter = raw.iter();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix(&format!("{PACKAGE_LONG_FLAG}=")) {
            return Some(value.to_string());
        }
        if arg == PACKAGE_LONG_FLAG || arg == PACKAGE_SHORT_FLAG {
            return iter.next().cloned();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|arg| (*arg).to_string()).collect()
    }

    #[test]
    fn extract_package_arg_long_with_separate_value() {
        let raw = args(&["cot", "--release", "--package", "blog", "check"]);

        assert_eq!(extract_package_arg(&raw), Some("blog".to_string()));
    }

    #[test]
    fn extract_package_arg_long_with_equals_value() {
        let raw = args(&["cot", "--package=blog", "check"]);

        assert_eq!(extract_package_arg(&raw), Some("blog".to_string()));
    }

    #[test]
    fn extract_package_arg_short_with_value() {
        let raw = args(&["cot", "-p", "blog", "check"]);

        assert_eq!(extract_package_arg(&raw), Some("blog".to_string()));
    }

    #[test]
    fn extract_package_arg_returns_first_package_flag() {
        let raw = args(&["cot", "-p", "first", "--package", "second", "check"]);

        assert_eq!(extract_package_arg(&raw), Some("first".to_string()));
    }

    #[test]
    fn extract_package_arg_missing_value_returns_none() {
        let raw = args(&["cot", "check", "-p"]);

        assert_eq!(extract_package_arg(&raw), None);
    }

    #[test]
    fn extract_package_arg_absent_returns_none() {
        let raw = args(&["cot", "--release", "check"]);

        assert_eq!(extract_package_arg(&raw), None);
    }
}
