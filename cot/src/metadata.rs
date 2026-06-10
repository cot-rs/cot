use clap::Command;
use serde::{Deserialize, Serialize};

pub const METADATA_FLAG: &str = "--metadata";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectMetadata {
    // pub version: u32,
    pub binary_name: String,
    pub commands: Vec<CommandMeta>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommandMeta {
    pub name: String,
    pub about: Option<String>,
    pub aliases: Vec<String>,
    pub subcommands: Vec<CommandMeta>,
}

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
        about: cmd.get_about().map(|s| s.to_string()),
        aliases: cmd.get_all_aliases().map(|s| s.to_string()).collect(),
        subcommands: cmd
            .get_subcommands()
            .filter(|subcmd| !subcmd.is_hide_set())
            .map(extract_command)
            .collect(),
    }
}
