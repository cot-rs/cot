use cot_cli::test_harness::standard_project;
use insta_cmd::assert_cmd_snapshot;

use crate::snapshot_testing::{GENERIC_FILTERS, TEMP_PATH_FILTERS, cot_cli_path, cot_cmd_in};

#[test]
fn check_forwards_to_project_binary() {
    let project = standard_project(cot_cli_path()).unwrap();
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(project.cot_cmd(&["check"])) }
    );
}

#[test]
fn double_dash_delimiter_fails_with_unsupported_flag_name() {
    let project = standard_project(cot_cli_path()).unwrap();
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(project.cot_cmd(&["check", "--", "--build"])) }
    );
}

#[test]
fn unrecognized_command_reports_unknown_command() {
    let project = standard_project(cot_cli_path()).unwrap();
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(project.cot_cmd(&["banana"])) }
    );
}

#[test]
fn check_with_no_project_binary_reports_build_hint() {
    let tempdir = tempfile::TempDir::new().unwrap();
    insta::with_settings!(
        { filters => [GENERIC_FILTERS, TEMP_PATH_FILTERS].concat() },
        { assert_cmd_snapshot!(cot_cmd_in(&["check"], tempdir.path())) }
    );
}
