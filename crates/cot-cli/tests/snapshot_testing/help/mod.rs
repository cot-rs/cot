use super::*;

#[test]
fn no_args() {
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(cot_cli!()) });
}

#[test]
fn short_help() {
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(cot_cli!("-h")) }
    );
}

#[test]
fn long_help() {
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(cot_cli!("--help")) }
    );
}

#[test]
fn help() {
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(cot_cli!("help")) }
    );
}

#[test]
fn help_new() {
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(cot_cli!("help", "new")) }
    );
}

#[test]
fn help_migration() {
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(cot_cli!("help", "migration")) }
    );
}

#[test]
fn help_migration_list() {
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(cot_cli!("help", "migration", "list")) }
    );
}

#[test]
fn help_migration_make() {
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(cot_cli!("help", "migration", "make")) }
    );
}

#[test]
fn help_cli_manpages() {
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(cot_cli!("help", "cli", "manpages")) }
    );
}

#[test]
fn help_cli_completions() {
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(cot_cli!("help", "cli", "completions")) }
    );
}
