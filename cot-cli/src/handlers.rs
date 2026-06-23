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
             Hint: run `cargo build` first, or `cargo build --release`."
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
    let mut cmd = combined_help_command(project);

    cmd.print_long_help()?;
    println!();
    Ok(())
}

fn combined_help_command(project: Option<&ProjectBinary>) -> clap::Command {
    let mut cmd = Cli::command();

    if let Some(proj) = project {
        for meta_cmd in &proj.metadata.commands {
            cmd = cmd.subcommand(build_clap_subcommand(meta_cmd));
        }
    }

    cmd
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

    #[test]
    fn external_command_without_project_reports_build_hint() {
        let result = handle_external(vec![OsString::from("serve")], None, false);

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("Unknown command `serve`"));
        assert!(message.contains("run `cargo build` first"));
    }

    #[test]
    fn external_command_unknown_to_project_reports_unknown_command() {
        let project = ProjectBinary {
            path: PathBuf::from("target/debug/example"),
            metadata: cot::metadata::ProjectMetadata {
                binary_name: "example".to_string(),
                commands: vec![CommandMeta {
                    name: "check".to_string(),
                    about: None,
                    aliases: vec![],
                    subcommands: vec![],
                }],
            },
        };

        let result = handle_external(vec![OsString::from("foo")], Some(project), false);

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("Unknown command `foo`"));
        assert!(message.contains("cot --help"));
    }

    #[test]
    fn build_clap_subcommand_preserves_about_aliases_and_nested_subcommands() {
        let meta = CommandMeta {
            name: "migration".to_string(),
            about: Some("Migration commands".to_string()),
            aliases: vec!["database".to_string()],
            subcommands: vec![CommandMeta {
                name: "rollback".to_string(),
                about: Some("Rollback migrations".to_string()),
                aliases: vec!["rbk".to_string()],
                subcommands: vec![],
            }],
        };

        let cmd = build_clap_subcommand(&meta);

        assert_eq!(cmd.get_name(), "migration");
        assert_eq!(cmd.get_about().unwrap().to_string(), "Migration commands");
        assert!(cmd.get_all_aliases().any(|alias| alias == "database"));
        let nested = cmd
            .get_subcommands()
            .find(|subcommand| subcommand.get_name() == "rollback")
            .unwrap();
        assert_eq!(
            nested.get_about().unwrap().to_string(),
            "Rollback migrations"
        );
        assert!(nested.get_all_aliases().any(|alias| alias == "rbk"));
    }

    #[test]
    fn combined_help_command_includes_project_commands_and_builtin_commands() {
        let project = ProjectBinary {
            path: PathBuf::from("target/debug/example"),
            metadata: cot::metadata::ProjectMetadata {
                binary_name: "example".to_string(),
                commands: vec![CommandMeta {
                    name: "health".to_string(),
                    about: Some("Check the server health".to_string()),
                    aliases: vec![],
                    subcommands: vec![],
                }],
            },
        };

        let cmd = combined_help_command(Some(&project));

        assert!(
            cmd.get_subcommands()
                .any(|subcommand| subcommand.get_name() == "new")
        );
        let health = cmd
            .get_subcommands()
            .find(|subcommand| subcommand.get_name() == "health")
            .unwrap();
        assert_eq!(
            health.get_about().unwrap().to_string(),
            "Check the server health"
        );
    }
}
