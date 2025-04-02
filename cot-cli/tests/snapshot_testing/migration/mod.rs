use std::io::Write;

use clap_verbosity_flag::{OffLevel, Verbosity};
use cot_cli::migration_generator::MigrationGeneratorOptions;
use cot_cli::{migration_generator, test_utils};

use super::*;

#[test]
#[expect(clippy::cast_possible_truncation)]
fn migration_list_empty() {
    let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
    test_utils::make_package(temp_dir.path()).unwrap();

    let mut cmd = cot_cli!("migration", "list");
    cmd.current_dir(temp_dir.path());

    for (idx, mut cli) in cot_clis_with_verbosity(&cmd).into_iter().enumerate() {
        let filter = Verbosity::<OffLevel>::new(idx as u8, 0).filter();

        insta::with_settings!(
            { description => format!("Verbosity level: {filter}") },
            { assert_cmd_snapshot!(cli); }
        );
    }
}

#[test]
#[expect(clippy::cast_possible_truncation)]
fn migration_list_existing() {
    let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
    test_utils::make_package(temp_dir.path()).unwrap();
    let mut main = std::fs::OpenOptions::new()
        .append(true)
        .open(temp_dir.path().join("src").join("main.rs"))
        .unwrap();
    write!(
        main,
        "#[model]\nstruct Test {{\n#[model(primary_key)]\nid: Auto<i32>\n}}"
    )
    .unwrap();
    migration_generator::make_migrations(
        temp_dir.path(),
        MigrationGeneratorOptions {
            app_name: None,
            output_dir: None,
        },
    )
    .unwrap();

    let mut cmd = cot_cli!("migration", "list");
    cmd.current_dir(temp_dir.path());

    for (idx, mut cli) in cot_clis_with_verbosity(&cmd).into_iter().enumerate() {
        let filter = Verbosity::<OffLevel>::new(idx as u8, 0).filter();

        insta::with_settings!(
            {
                description => format!("Verbosity level: {filter}"),
                filters => [GENERIC_FILTERS, TEMP_PATH_FILTERS, &[
                    (r"(?m)^(cot-test)[^ \t]+", "$1-PROJECT-NAME")  // Remove temp dir name
                ]].concat()
            },
            { assert_cmd_snapshot!(cli); }
        );
    }
}

#[test]
#[expect(clippy::cast_possible_truncation)]
fn migration_make_no_models() {
    let cmd = cot_cli!("migration", "make");
    for (idx, mut cli) in cot_clis_with_verbosity(&cmd).into_iter().enumerate() {
        let filter = Verbosity::<OffLevel>::new(idx as u8, 0).filter();

        let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        test_utils::make_package(temp_dir.path()).unwrap();

        insta::with_settings!(
            {
                description => format!("Verbosity level: {filter}"),
                filters => [GENERIC_FILTERS, TEMP_PATH_FILTERS, TEMP_PROJECT_FILTERS].concat()
            },
            { assert_cmd_snapshot!(cli.current_dir(temp_dir.path())) }
        );
    }
}

#[test]
#[expect(clippy::cast_possible_truncation)]
fn migration_make_existing_model() {
    let cmd = cot_cli!("migration", "make");
    for (idx, mut cli) in cot_clis_with_verbosity(&cmd).into_iter().enumerate() {
        let filter = Verbosity::<OffLevel>::new(idx as u8, 0).filter();

        let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        test_utils::make_package(temp_dir.path()).unwrap();
        let mut main = std::fs::OpenOptions::new()
            .append(true)
            .open(temp_dir.path().join("src").join("main.rs"))
            .unwrap();
        write!(
            main,
            "#[model]\nstruct Test {{\n#[model(primary_key)]\nid: Auto<i32>\n}}"
        )
        .unwrap();

        insta::with_settings!(
            {
                description => format!("Verbosity level: {filter}"),
                filters => [GENERIC_FILTERS, TEMP_PATH_FILTERS, TEMP_PROJECT_FILTERS].concat()
            },
            { assert_cmd_snapshot!(cli.current_dir(temp_dir.path())) }
        );
    }
}
