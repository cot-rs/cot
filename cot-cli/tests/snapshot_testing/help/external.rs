use cot_cli::test_harness::standard_project;
use insta_cmd::assert_cmd_snapshot;

use crate::snapshot_testing::{GENERIC_FILTERS, cot_cli_path};

#[test]
fn top_level_help_merges_real_project_commands() {
    let project = standard_project(cot_cli_path()).unwrap();
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(project.cot_cmd(&["--build", "--help"])) }
    );
}

#[test]
fn migration_help_merges_real_rollback_subcommand() {
    let project = standard_project(cot_cli_path()).unwrap();
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(project.cot_cmd(&["--build", "migration", "--help"])) }
    );
}

#[test]
fn migration_rollback_help_succeeds_proving_command_is_reachable() {
    let project = standard_project(cot_cli_path()).unwrap();
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(project.cot_cmd(&["--build", "migration", "rollback", "--help"])) }
    );
}

#[test]
fn migration_unknown_subcommand_fails_cleanly() {
    let project = standard_project(cot_cli_path()).unwrap();
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(project.cot_cmd(&["--build", "migration", "unknown", "--help"])) }
    );
}

#[test]
fn check_help_shows_real_task() {
    let project = standard_project(cot_cli_path()).unwrap();
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(project.cot_cmd(&["check", "--help"])) }
    );
}

#[test]
fn custom_registered_task_appears_in_top_level_help() {
    let project = standard_project(cot_cli_path()).unwrap();
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(project.cot_cmd(&["--build", "--help"])) }
    );
}

#[test]
fn custom_task_help_shows_reconstructed_args() {
    let project = standard_project(cot_cli_path()).unwrap();
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(project.cot_cmd(&["--build", "frobnicate", "--help"])) }
    );
}

#[test]
fn nested_custom_group_help_merges_correctly() {
    let project = standard_project(cot_cli_path()).unwrap();
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(project.cot_cmd(&["--build", "fixture-group", "--help"])) }
    );
}
