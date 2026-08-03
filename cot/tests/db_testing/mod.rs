mod fields;
mod migrations;
mod query;
mod relations;

macro_rules! run_migrations {
    ( $db:ident, $( $operations:ident ),* ) => {
        struct TestMigration;

        impl cot::db::migrations::Migration for TestMigration {
            const APP_NAME: &'static str = "cot";
            const DEPENDENCIES: &'static [cot::db::migrations::MigrationDependency] = &[];
            const MIGRATION_NAME: &'static str = "test_migration";
            const OPERATIONS: &'static [Operation] = &[ $($operations),* ];
        }

        cot::db::migrations::MigrationEngine::new(
            cot::db::migrations::wrap_migrations(&[&TestMigration])
        )
            .unwrap()
            .run(&**$db)
            .await
            .unwrap();
    };
}
pub(super) use run_migrations;
