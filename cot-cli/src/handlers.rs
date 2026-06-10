use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;

use anyhow::Context;
use clap::CommandFactory;
use cot::metadata::CommandMeta;

use crate::args::{
    Cli, CompletionsArgs, ManpagesArgs, MigrationListArgs, MigrationMakeArgs, MigrationNewArgs,
    ProjectNewArgs,
};
use crate::migration_generator::{
    MigrationGeneratorOptions, create_new_migration, list_migrations, make_migrations,
};
use crate::new_project::{CotSource, new_project};
use crate::project::ProjectBinary;

pub fn handle_new_project(
    ProjectNewArgs { path, name, source }: ProjectNewArgs,
) -> anyhow::Result<()> {
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

pub fn handle_migration_list(MigrationListArgs { path }: MigrationListArgs) -> anyhow::Result<()> {
    let path = path.unwrap_or(PathBuf::from("."));
    let migrations = list_migrations(&path).with_context(|| "unable to list migrations")?;
    for (app_name, migs) in migrations {
        for mig in migs {
            println!("{app_name}\t{mig}");
        }
    }

    Ok(())
}

pub fn handle_migration_make(
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

pub fn handle_migration_new(
    MigrationNewArgs {
        name,
        path,
        app_name,
    }: MigrationNewArgs,
) -> anyhow::Result<()> {
    let path = path.unwrap_or(PathBuf::from("."));
    let options = MigrationGeneratorOptions {
        app_name,
        output_dir: None,
    };
    create_new_migration(&path, &name, options).with_context(|| "unable to create migration")
}

pub fn handle_cli_manpages(
    ManpagesArgs { output_dir, create }: ManpagesArgs,
) -> anyhow::Result<()> {
    let output_dir = output_dir.unwrap_or(PathBuf::from("."));
    if create {
        std::fs::create_dir_all(&output_dir).context("unable to create output directory")?;
    }
    clap_mangen::generate_to(Cli::command(), output_dir)
        .context("unable to generate manpages in output directory")
}

#[expect(clippy::unnecessary_wraps)] // return Result<()> for consistency
pub fn handle_cli_completions(CompletionsArgs { shell }: CompletionsArgs) -> anyhow::Result<()> {
    generate_completions(shell, &mut std::io::stdout());

    Ok(())
}

pub fn handle_external(
    args: Vec<OsString>,
    project: Option<ProjectBinary>,
    _release: bool,
) -> anyhow::Result<()> {
    let subcmd = args[0].to_string_lossy();

    let Some(proj) = project else {
        anyhow::bail!(
            "Unknown command `{subcmd}` and no project binary was found in target/.\n\
             Hint: run `cargo build` first, or `cargo build --release` with --release."
        );
    };

    let known = proj
        .metadata
        .commands
        .iter()
        .any(|c| c.name == subcmd.as_ref() || c.aliases.iter().any(|a| a == subcmd.as_ref()));

    if !known {
        anyhow::bail!(
            "Unknown command `{subcmd}`.\n\
             Run `cot --help` to see all available commands."
        );
    }

    exec(proj, args)
}

fn exec(proj: ProjectBinary, args: Vec<OsString>) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        let err = std::process::Command::new(&proj.path).args(&args).exec();
        anyhow::bail!("Failed to exec {}: {err}", proj.path.display());
    }

    #[cfg(not(unix))]
    {
        let status = std::process::Command::new(&proj.path)
            .args(&args)
            .status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

/// Build a fresh [`clap::Command`] and inject the project's subcommands into
/// it before printing.
pub fn handle_combined_help(project: Option<&ProjectBinary>) -> anyhow::Result<()> {
    let mut cmd = Cli::command();

    if let Some(proj) = project {
        for meta_cmd in &proj.metadata.commands {
            cmd = cmd.subcommand(build_clap_subcommand(meta_cmd));
        }
    }

    cmd.print_long_help()?;
    println!();
    Ok(())
}

fn build_clap_subcommand(meta: &CommandMeta) -> clap::Command {
    let mut cmd = clap::Command::new(&meta.name);

    if let Some(about) = &meta.about {
        cmd = cmd.about(about.clone());
    }

    for alias in &meta.aliases {
        cmd = cmd.visible_alias(alias.clone());
    }

    for sub in &meta.subcommands {
        cmd = cmd.subcommand(build_clap_subcommand(sub));
    }

    cmd
}

fn generate_completions(shell: clap_complete::Shell, writer: &mut impl std::io::Write) {
    clap_complete::generate(shell, &mut Cli::command(), "cot", writer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::CotSourceArgs;

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
    fn migration_new_wrong_directory() {
        let args = MigrationNewArgs {
            name: "test_migration".to_string(),
            path: Some(PathBuf::from("nonexistent")),
            app_name: None,
        };

        let result = handle_migration_new(args);

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
