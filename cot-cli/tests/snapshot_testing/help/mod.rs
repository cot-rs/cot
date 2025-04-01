use super::*;

#[test]
fn no_args() {
    assert_cmd_snapshot!(cot_cli!());
}

#[test]
fn short_help() {
    assert_cmd_snapshot!(cot_cli!("-h"));
}

#[test]
fn long_help() {
    assert_cmd_snapshot!(cot_cli!("--help"));
}

#[test]
fn version() {
    assert_cmd_snapshot!(cot_cli!("--version"));
}

#[test]
fn help() {
    assert_cmd_snapshot!(cot_cli!("help"));
}

#[test]
fn help_new() {
    assert_cmd_snapshot!(cot_cli!("help", "new"));
}

#[test]
fn help_migration() {
    assert_cmd_snapshot!(cot_cli!("help", "migration"));
}

#[test]
fn help_migration_list() {
    assert_cmd_snapshot!(cot_cli!("help", "migration", "list"));
}

#[test]
fn help_migration_make() {
    assert_cmd_snapshot!(cot_cli!("help", "migration", "make"));
}

#[test]
fn help_cli_manpages() {
    assert_cmd_snapshot!(cot_cli!("help", "cli", "manpages"));
}

#[test]
fn help_cli_completions() {
    assert_cmd_snapshot!(cot_cli!("help", "cli", "completions"));
}
