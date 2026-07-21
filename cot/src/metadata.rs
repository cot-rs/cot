//! Metadata exported by Cot project binaries for the proxying `cot` CLI.

use clap::Command;
use serde::{Deserialize, Serialize};

/// Flag used to ask a Cot project binary to print its CLI metadata as JSON.
pub const METADATA_FLAG: &str = "--metadata";

/// Metadata describing the commands exposed by a Cot project binary.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectMetadata {
    /// Name of the project binary that produced the metadata.
    pub binary_name: String,
    /// Top-level commands exposed by the project binary.
    pub commands: Vec<CommandMeta>,
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
}

/// Extract proxyable command metadata from a clap command definition.
pub fn extract(cmd: &Command) -> ProjectMetadata {
    ProjectMetadata {
        binary_name: cmd.get_name().to_string(),
        commands: cmd
            .get_subcommands()
            .filter(|subcmd| !subcmd.is_hide_set())
            .map(extract_command)
            .collect(),
    }
}

fn extract_command(cmd: &Command) -> CommandMeta {
    CommandMeta {
        name: cmd.get_name().to_string(),
        about: cmd.get_about().map(ToString::to_string),
        aliases: cmd.get_all_aliases().map(ToString::to_string).collect(),
        subcommands: cmd
            .get_subcommands()
            .filter(|subcmd| !subcmd.is_hide_set())
            .map(extract_command)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract() {
        let command = Command::new("demo")
            .subcommand(Command::new("serve").about("Serve requests"))
            .subcommand(Command::new("secret").hide(true));

        let metadata = extract(&command);

        assert_eq!(metadata.binary_name, "demo");
        assert_eq!(metadata.commands.len(), 1);
        assert_eq!(metadata.commands[0].name, "serve");
        assert_eq!(
            metadata.commands[0].about.as_deref(),
            Some("Serve requests")
        );
    }

    #[test]
    fn test_extract_command_with_visible_aliases() {
        let command = Command::new("demo").subcommand(
            Command::new("database")
                .visible_alias("db")
                .subcommand(Command::new("migrate").visible_alias("mig"))
                .subcommand(Command::new("internal").hide(true)),
        );

        let metadata = extract(&command);
        let database = &metadata.commands[0];

        assert_eq!(database.name, "database");
        assert_eq!(database.aliases, vec!["db"]);
        assert_eq!(database.subcommands.len(), 1);
        assert_eq!(database.subcommands[0].name, "migrate");
        assert_eq!(database.subcommands[0].aliases, vec!["mig"]);
    }

    #[test]
    fn test_extract_command_with_no_about() {
        let command = Command::new("demo").subcommand(Command::new("plain"));

        let metadata = extract(&command);

        assert_eq!(metadata.commands[0].about, None);
    }
}
