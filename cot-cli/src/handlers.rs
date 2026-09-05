use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::PathBuf;

use anyhow::Context;
use clap::CommandFactory;
use cot::metadata::CommandMeta;
use cot::utils::cli::{StatusType, print_status_msg};

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
    command_path: &[String],
    remaining_args: &[OsString],
    project: Option<ProjectBinary>,
    _release: bool,
) -> anyhow::Result<()> {
    let subcmd = command_path.join(" ");

    let Some(proj) = project else {
        anyhow::bail!(
            "unknown command `{subcmd}` and no project binary was found in the `target` dir.\n\
             Hint: run `cargo build` first, or pass `cot --build {subcmd}` to build it automatically."
        );
    };

    match &proj.metadata {
        Some(meta) if command_path_exists(&meta.commands, command_path) => {
            // command is known, proceed to exec
        }
        Some(_) => {
            // metadata found but command is not known
            anyhow::bail!(
                "unknown command `{subcmd}`. Run `cot --help` to see available commands."
            );
        }
        None => {
            // The metadata retrieval from the binary most likely failed or didnt exist so
            // theres no way to validate the command exists here. We forward the command
            // unconditionally and let the binary handle it.
            print_status_msg(
                StatusType::Warning,
                &format!(
                    "could not obtain metadata for `{}`; forwarding `{subcmd}` command directly",
                    proj.path.display()
                ),
            );
        }
    }

    let full_args: Vec<OsString> = command_path
        .iter()
        .map(OsString::from)
        .chain(remaining_args.iter().cloned())
        .collect();

    exec(&proj, &full_args)
}

fn command_path_exists(commands: &[CommandMeta], path: &[String]) -> bool {
    let mut current: &[CommandMeta] = commands;

    for segment in path {
        let found = current
            .iter()
            .find(|c| c.name == *segment || c.aliases.iter().any(|a| a == segment));

        match found {
            Some(cmd) => current = &cmd.subcommands,
            None => return false,
        }
    }

    true
}

fn exec(proj: &ProjectBinary, args: &[OsString]) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        let err = std::process::Command::new(&proj.path).args(args).exec();
        anyhow::bail!("Failed to exec {}: {err}", proj.path.display());
    }

    #[cfg(not(unix))]
    {
        // Windows has no equivalent of POSIX `execve` that replaces the current
        // process in place. The best we can do is spawn the binary as a
        // child and block here until it exits
        let status = std::process::Command::new(&proj.path).args(args).status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
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
        let result = handle_external(&["serve".to_string()], &[], None, false);

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("unknown command `serve`"));
        assert!(message.contains("run `cargo build` first"));
    }

    #[test]
    fn external_command_unknown_to_project_reports_unknown_command() {
        let project = ProjectBinary {
            path: PathBuf::from("target/debug/example"),
            metadata: Some(cot::metadata::ProjectMetadata {
                version: cot::metadata::METADATA_SCHEMA_VERSION,
                binary_name: "example".to_string(),
                commands: vec![CommandMeta {
                    name: "check".to_string(),
                    about: None,
                    aliases: vec![],
                    subcommands: vec![],
                    args: vec![],
                }],
            }),
        };

        let result = handle_external(&["foo".to_string()], &[], Some(project), false);

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("unknown command `foo`"));
        assert!(message.contains("cot --help"));
    }

    #[test]
    fn external_command_nested_path_unknown_reports_unknown_command() {
        let project = ProjectBinary {
            path: PathBuf::from("target/debug/example"),
            metadata: Some(cot::metadata::ProjectMetadata {
                version: cot::metadata::METADATA_SCHEMA_VERSION,
                binary_name: "example".to_string(),
                commands: vec![CommandMeta {
                    name: "migration".to_string(),
                    about: None,
                    aliases: vec![],
                    subcommands: vec![CommandMeta {
                        name: "rollback".to_string(),
                        about: None,
                        aliases: vec![],
                        subcommands: vec![],
                        args: vec![],
                    }],
                    args: vec![],
                }],
            }),
        };

        let result = handle_external(
            &["migration".to_string(), "nonexistent".to_string()],
            &[],
            Some(project),
            false,
        );

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("unknown command `migration nonexistent`"));
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: can't call foreign function `execvp` on OS `linux`"
    )]
    #[cfg(unix)]
    fn known_nested_command_attempts_exec_and_fails_when_binary_missing() {
        let project = ProjectBinary {
            path: PathBuf::from("/nonexistent/binary/path"),
            metadata: Some(cot::metadata::ProjectMetadata {
                version: cot::metadata::METADATA_SCHEMA_VERSION,
                binary_name: "example".to_string(),
                commands: vec![CommandMeta {
                    name: "migration".to_string(),
                    about: None,
                    aliases: vec![],
                    subcommands: vec![CommandMeta {
                        name: "rollback".to_string(),
                        about: None,
                        aliases: vec![],
                        subcommands: vec![],
                        args: vec![],
                    }],
                    args: vec![],
                }],
            }),
        };

        let result = handle_external(
            &["migration".to_string(), "rollback".to_string()],
            &[OsString::from("my_migration"), OsString::from("--dry-run")],
            Some(project),
            false,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to exec"));
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: can't call foreign function `execvp` on OS `linux`"
    )]
    #[cfg(unix)]
    fn missing_metadata_forwards_blindly_and_attempts_exec() {
        let project = ProjectBinary {
            path: PathBuf::from("/nonexistent/binary/path"),
            metadata: None,
        };

        let result = handle_external(&["anything".to_string()], &[], Some(project), false);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to exec"));
    }

    #[test]
    fn command_path_exists_finds_nested_command() {
        let commands = vec![CommandMeta {
            name: "migration".to_string(),
            about: None,
            aliases: vec![],
            subcommands: vec![CommandMeta {
                name: "rollback".to_string(),
                about: None,
                aliases: vec![],
                subcommands: vec![],
                args: vec![],
            }],
            args: vec![],
        }];

        assert!(command_path_exists(
            &commands,
            &["migration".to_string(), "rollback".to_string()]
        ));
    }

    #[test]
    fn command_path_exists_matches_via_alias() {
        let commands = vec![CommandMeta {
            name: "migration".to_string(),
            about: None,
            aliases: vec!["mig".to_string()],
            subcommands: vec![],
            args: vec![],
        }];

        assert!(command_path_exists(&commands, &["mig".to_string()]));
    }

    #[test]
    fn command_path_exists_rejects_missing_nested_command() {
        let commands = vec![CommandMeta {
            name: "migration".to_string(),
            about: None,
            aliases: vec![],
            subcommands: vec![CommandMeta {
                name: "rollback".to_string(),
                about: None,
                aliases: vec![],
                subcommands: vec![],
                args: vec![],
            }],
            args: vec![],
        }];

        assert!(!command_path_exists(
            &commands,
            &["migration".to_string(), "nonexistent".to_string()]
        ));
    }

    #[test]
    fn command_path_exists_empty_path_is_true() {
        assert!(command_path_exists(&[], &[]));
    }
}
