use cot_cli::test_harness::{CotAppBuilder, CotProject, CotProjectBuilder};

use super::*;

const MIGRATION_FILTERS: &[(&str, &str)] = &[
    // cargo output of building the project binary
    (
        r"(?m)^ *(Adding|Blocking|Compiling|Finished|Locking|Updating)\b.*\n",
        "",
    ),
    (r"\bcot_cli_test_\d+_\d+__", "[PROJECT_NAME]__"),
    (r"(?ms)^warning: .*?generated \d+ warnings?[^\n]*\n", ""),
    (r"cot-test-harness-[^/]+", "TEMP_PATH"),
    (r"on \d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\+00:00", "on DATE"),
    (r"auto_\d{8}_\d{6}", "auto_DATE"),
];

fn filters() -> Vec<(&'static str, &'static str)> {
    [GENERIC_FILTERS, TEMP_PATH_FILTERS, MIGRATION_FILTERS].concat()
}

/// A project with an app that registers its migrations, and the default
/// `DefaultTestModel` model.
fn migration_project() -> CotProject {
    let app = CotAppBuilder::new("cot_test_migrations")
        .migrations("cot::db::migrations::wrap_migrations(migrations::MIGRATIONS)")
        .build();
    CotProjectBuilder::new(cot_cli_path())
        .app(app)
        .build()
        .unwrap()
}

fn read_migration(project: &CotProject, name: &str) -> String {
    std::fs::read_to_string(project.path().join("src").join(name)).unwrap()
}

#[test]
fn migration_make() {
    let project = migration_project();

    insta::with_settings!({ filters => filters() }, {
        assert_cmd_snapshot!(
            "make_initial",
            project.cot_cmd_shared_target(&["migration", "make"])
        );
        insta::assert_snapshot!(
            "make_initial_migration",
            read_migration(&project, "migrations/m_0001_initial.rs")
        );
        insta::assert_snapshot!(
            "make_initial_migrations_module",
            read_migration(&project, "migrations.rs")
        );

        assert_cmd_snapshot!(
            "make_no_changes",
            project.cot_cmd_shared_target(&["migration", "make"])
        );
        assert_cmd_snapshot!(
            "list",
            project.cot_cmd_shared_target(&["migration", "list"])
        );
    });
}

#[test]
fn migration_new() {
    let project = migration_project();
    let make = project
        .cot_cmd_shared_target(&["migration", "make"])
        .output()
        .unwrap();
    assert!(make.status.success(), "{make:?}");

    insta::with_settings!({ filters => filters() }, {
        assert_cmd_snapshot!(
            "new",
            project.cot_cmd_shared_target(&["migration", "new", "custom"])
        );
        insta::assert_snapshot!(
            "new_migration",
            read_migration(&project, "migrations/m_0002_custom.rs")
        );
    });
}
