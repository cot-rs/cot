use cot_cli::test_harness::standard_project;
use insta_cmd::assert_cmd_snapshot;

use crate::snapshot_testing::{GENERIC_FILTERS, cot_cli_path};

#[test]
fn migration_list_forwards_to_project_binary() {
    let project = standard_project(cot_cli_path()).unwrap();
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(project.cot_cmd(&["migration", "list"])) }
    );
}
