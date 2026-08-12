//! Metadata exported by Cot project binaries for the proxying `cot` CLI.

use clap::{Arg, Command};
use serde::{Deserialize, Serialize};

/// The current version of the `ProjectMetadata` JSON schema.
pub const METADATA_SCHEMA_VERSION: u32 = 1;

/// Flag used to ask a Cot project binary to print its CLI metadata as JSON.
pub const METADATA_FLAG: &str = "--cot-internal-cli-metadata";

/// Metadata describing the commands exposed by a Cot project binary.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectMetadata {
    /// Schema version this metadata was serialized with.
    pub version: u32,
    /// Name of the project binary that produced the metadata.
    pub binary_name: String,
    /// Top-level commands exposed by the project binary.
    pub commands: Vec<CommandMeta>,
}

impl ProjectMetadata {
    /// Create new Project metadata
    pub fn new(cmd: &Command) -> Self {
        ProjectMetadata {
            version: METADATA_SCHEMA_VERSION,
            binary_name: cmd.get_name().to_string(),
            commands: cmd
                .get_subcommands()
                .filter(|subcmd| !subcmd.is_hide_set())
                .map(CommandMeta::from)
                .collect(),
        }
    }
}

impl From<&Command> for ProjectMetadata {
    fn from(cmd: &Command) -> Self {
        Self::new(cmd)
    }
}

/// Arguments for a CLI command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgMeta {
    /// Argument Name.
    pub name: String,
    /// long option name.
    pub long: Option<String>,
    /// short option name.
    pub short: Option<char>,
    /// Help text for the argument.
    pub help: Option<String>,
    /// Whether the argument is required.
    pub required: bool,
    /// Whether the argument is a positional argument.
    pub is_positional: bool,
    /// Whether the argument takes a value.
    pub takes_value: bool,
    /// The value name for this argument.
    pub value_name: Option<String>,
}

impl From<&Arg> for ArgMeta {
    fn from(arg: &Arg) -> Self {
        Self {
            name: arg.get_id().to_string(),
            long: arg.get_long().map(str::to_string),
            short: arg.get_short(),
            help: arg.get_help().map(ToString::to_string),
            required: arg.is_required_set(),
            is_positional: arg.is_positional(),
            takes_value: arg.get_num_args().is_some_and(|n| n.takes_values()),
            value_name: arg
                .get_value_names()
                .and_then(|v| v.first())
                .map(ToString::to_string),
        }
    }
}

/// Metadata for a single CLI command.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommandMeta {
    /// Command name.
    pub name: String,
    /// Optional command description.
    pub about: Option<String>,
    /// Visible aliases accepted by the command.
    pub aliases: Vec<String>,
    /// Nested subcommands exposed by this command.
    pub subcommands: Vec<CommandMeta>,
    /// Arguments supported by the command.
    pub args: Vec<ArgMeta>,
}

impl From<&Command> for CommandMeta {
    fn from(cmd: &Command) -> Self {
        CommandMeta {
            name: cmd.get_name().to_string(),
            about: cmd.get_about().map(ToString::to_string),
            aliases: cmd.get_all_aliases().map(ToString::to_string).collect(),
            subcommands: cmd
                .get_subcommands()
                .filter(|subcmd| !subcmd.is_hide_set())
                .map(CommandMeta::from)
                .collect(),
            args: cmd
                .get_arguments()
                .filter(|a| a.get_id() != "help" && a.get_id() != "version")
                .map(ArgMeta::from)
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_metadata_from() {
        let command = Command::new("demo")
            .subcommand(Command::new("serve").about("Serve requests"))
            .subcommand(Command::new("secret").hide(true));

        let metadata = ProjectMetadata::from(&command);

        assert_eq!(metadata.binary_name, "demo");
        assert_eq!(metadata.commands.len(), 1);
        assert_eq!(metadata.commands[0].name, "serve");
        assert_eq!(
            metadata.commands[0].about.as_deref(),
            Some("Serve requests")
        );
    }

    #[test]
    fn test_from_command_with_visible_aliases() {
        let command = Command::new("demo").subcommand(
            Command::new("database")
                .visible_alias("db")
                .subcommand(Command::new("migrate").visible_alias("mig"))
                .subcommand(Command::new("internal").hide(true)),
        );

        let metadata = ProjectMetadata::from(&command);
        let database = &metadata.commands[0];

        assert_eq!(database.name, "database");
        assert_eq!(database.aliases, vec!["db"]);
        assert_eq!(database.subcommands.len(), 1);
        assert_eq!(database.subcommands[0].name, "migrate");
        assert_eq!(database.subcommands[0].aliases, vec!["mig"]);
    }

    #[test]
    fn test_from_command_with_no_about() {
        let command = Command::new("demo").subcommand(Command::new("plain"));

        let metadata = ProjectMetadata::from(&command);

        assert_eq!(metadata.commands[0].about, None);
    }

    #[test]
    fn command_meta_from_captures_args() {
        let command = Command::new("demo").subcommand(
            Command::new("rollback")
                .arg(
                    Arg::new("migration_name")
                        .value_name("MIGRATION_NAME")
                        .required(true),
                )
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .action(clap::ArgAction::SetTrue),
                ),
        );

        let metadata = ProjectMetadata::from(&command);
        let rollback = &metadata.commands[0];

        assert_eq!(rollback.args.len(), 2);

        let positional = rollback
            .args
            .iter()
            .find(|a| a.name == "migration_name")
            .unwrap();
        assert!(positional.is_positional);
        assert!(positional.required);
        assert_eq!(positional.value_name.as_deref(), Some("MIGRATION_NAME"));

        let flag = rollback.args.iter().find(|a| a.name == "dry-run").unwrap();
        assert!(!flag.is_positional);
        assert_eq!(flag.long.as_deref(), Some("dry-run"));
        assert!(!flag.takes_value);
    }

    #[test]
    fn command_meta_from_excludes_help_and_version_ids() {
        let command = Command::new("demo").subcommand(
            Command::new("sub")
                .arg(Arg::new("help").long("help"))
                .arg(Arg::new("version").long("version"))
                .arg(Arg::new("real").long("real")),
        );

        let metadata = ProjectMetadata::from(&command);
        let sub = &metadata.commands[0];

        assert_eq!(sub.args.len(), 1);
        assert_eq!(sub.args[0].name, "real");
    }

    #[test]
    fn arg_meta_from_flag_arg() {
        let arg = Arg::new("verbose")
            .short('v')
            .long("verbose")
            .action(clap::ArgAction::SetTrue);

        let meta = ArgMeta::from(&arg);

        assert_eq!(meta.name, "verbose");
        assert_eq!(meta.short, Some('v'));
        assert_eq!(meta.long.as_deref(), Some("verbose"));
        assert!(!meta.takes_value);
        assert!(!meta.is_positional);
    }

    #[test]
    fn arg_meta_from_positional_arg() {
        let arg = Arg::new("path").value_name("PATH").required(true);

        let meta = ArgMeta::from(&arg);

        assert!(meta.is_positional);
        assert!(meta.required);
        assert_eq!(meta.value_name.as_deref(), Some("PATH"));
    }
}
