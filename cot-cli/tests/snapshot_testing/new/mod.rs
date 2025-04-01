use clap_verbosity_flag::{OffLevel, Verbosity, VerbosityFilter};

use super::*;

#[test]
fn create_new_project() {
    let cmd = cot_cli!("new");
    for (idx, ref mut cli) in cot_clis_with_verbosity(cmd).into_iter().enumerate() {
        let tempdir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        let filter = Verbosity::<OffLevel>::new(idx as u8, 0).filter();

        let mut filters = get_logging_filters();
        filters.extend([
            (r"cot-test-[^/]+", "TEMP_PATH"), // Remove temp dir path
        ]);

        insta::with_settings!(
            {
                description => format!("Verbosity level: {filter}"),
                filters => filters
            },
            {
            assert_cmd_snapshot!(cli.arg(tempdir.path().join("project")));
            }
        );
    }
}

#[test]
fn create_new_project_with_custom_name() {
    let cmd = cot_cli!("new", "--name", "my_project");
    for (idx, ref mut cli) in cot_clis_with_verbosity(cmd).into_iter().enumerate() {
        let tempdir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        let filter = Verbosity::<OffLevel>::new(idx as u8, 0).filter();

        let mut filters = get_logging_filters();
        filters.extend([
            (r"cot-test-[^/]+", "TEMP_PATH"), // Remove temp dir path
        ]);

        insta::with_settings!(
            {
                description => format!("Verbosity level: {filter}"),
                filters => filters
            },
            {
            assert_cmd_snapshot!(cli.arg(tempdir.path().join("project")));
            }
        );
    }
}
