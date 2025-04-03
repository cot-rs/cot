use clap_verbosity_flag::{OffLevel, Verbosity};

use super::*;

#[test]
#[expect(clippy::cast_possible_truncation)]
fn create_new_project() {
    let cmd = cot_cli!("new");
    for (idx, ref mut cli) in cot_clis_with_verbosity(&cmd).into_iter().enumerate() {
        let tempdir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        let filter = Verbosity::<OffLevel>::new(idx as u8, 0).filter();

        insta::with_settings!(
            {
                description => format!("Verbosity level: {filter}"),
                filters => [GENERIC_FILTERS, TEMP_PATH_FILTERS, TEMP_PROJECT_FILTERS].concat()
            },
            {
                assert_cmd_snapshot!(cli.arg(tempdir.path().join("project")));
            }
        );
    }
}

#[test]
#[expect(clippy::cast_possible_truncation)]
fn create_new_project_with_custom_name() {
    let cmd = cot_cli!("new", "--name", "my_project");
    for (idx, ref mut cli) in cot_clis_with_verbosity(&cmd).into_iter().enumerate() {
        let tempdir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        let filter = Verbosity::<OffLevel>::new(idx as u8, 0).filter();

        insta::with_settings!(
            {
                description => format!("Verbosity level: {filter}"),
                filters => [GENERIC_FILTERS, TEMP_PATH_FILTERS, TEMP_PROJECT_FILTERS].concat()
            },
            {
                assert_cmd_snapshot!(cli.arg(tempdir.path().join("project")));
            }
        );
    }
}
