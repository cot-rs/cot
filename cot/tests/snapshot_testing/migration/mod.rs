use cot::App;
use cot::auth::db::DatabaseUserApp;
use cot::db::migrations::{
    Field, Migration, MigrationDependency, MigrationEngine, Operation, SyncDynMigration,
    wrap_migrations,
};
use cot::db::{Database, DatabaseField, Identifier};
use cot::session::db::SessionApp;
use cot::test::TestDatabase;

struct RollbackApp1Initial;

impl Migration for RollbackApp1Initial {
    const APP_NAME: &'static str = "rollback_app1";
    const MIGRATION_NAME: &'static str = "m_0001_initial";
    const DEPENDENCIES: &'static [MigrationDependency] = &[];
    const OPERATIONS: &'static [Operation] = &[Operation::create_model()
        .table_name(Identifier::new("rollback_single__first"))
        .fields(&[
            Field::new(Identifier::new("id"), <i32 as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
        ])
        .build()];
}

struct RollbackApp10002;

impl Migration for RollbackApp10002 {
    const APP_NAME: &'static str = "rollback_app1";
    const MIGRATION_NAME: &'static str = "m_0002_second";
    const DEPENDENCIES: &'static [MigrationDependency] = &[MigrationDependency::migration(
        "rollback_app1",
        "m_0001_initial",
    )];
    const OPERATIONS: &'static [Operation] = &[Operation::create_model()
        .table_name(Identifier::new("rollback_app1__second"))
        .fields(&[
            Field::new(Identifier::new("id"), <i32 as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
        ])
        .build()];
}

struct RollbackApp1003;

impl Migration for RollbackApp1003 {
    const APP_NAME: &'static str = "rollback_app1";
    const MIGRATION_NAME: &'static str = "m_0003_third";
    const DEPENDENCIES: &'static [MigrationDependency] = &[MigrationDependency::migration(
        "rollback_app1",
        "m_0002_second",
    )];
    const OPERATIONS: &'static [Operation] = &[Operation::create_model()
        .table_name(Identifier::new("rollback_single__third"))
        .fields(&[
            Field::new(Identifier::new("id"), <i32 as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
        ])
        .build()];
}

struct RollbackApp2Initial;

impl Migration for RollbackApp2Initial {
    const APP_NAME: &'static str = "rollback_app2";
    const MIGRATION_NAME: &'static str = "m_0001_initial";
    const DEPENDENCIES: &'static [MigrationDependency] = &[];
    const OPERATIONS: &'static [Operation] = &[Operation::create_model()
        .table_name(Identifier::new("rollback_app2__foo"))
        .fields(&[
            Field::new(Identifier::new("id"), <i32 as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
        ])
        .build()];
}

struct RollbackDependentInitial;

impl Migration for RollbackDependentInitial {
    const APP_NAME: &'static str = "rollback_dependent";
    const MIGRATION_NAME: &'static str = "m_0001_initial";
    const DEPENDENCIES: &'static [MigrationDependency] = &[MigrationDependency::migration(
        "rollback_app1",
        "m_0002_second",
    )];
    const OPERATIONS: &'static [Operation] = &[Operation::create_model()
        .table_name(Identifier::new("rollback_dependent__bar"))
        .fields(&[
            Field::new(Identifier::new("id"), <i32 as DatabaseField>::TYPE)
                .primary_key()
                .auto(),
        ])
        .build()];
}

async fn dry_run_snapshot(
    engine: &MigrationEngine,
    db: &Database,
    output: &mut Vec<u8>,
    migration_name: &str,
    app_name: &str,
) -> String {
    output.clear();
    engine
        .rollback_dry_run(db, migration_name, app_name, output)
        .await
        .unwrap();
    std::str::from_utf8(output).unwrap().to_owned()
}

#[cot_macros::dbtest]
async fn test_migration_engine_rollback_single_app(test_db: &mut TestDatabase) {
    let mut output = Vec::new();
    #[expect(trivial_casts)]
    let engine = MigrationEngine::new([
        &RollbackApp1Initial as &SyncDynMigration,
        &RollbackApp10002 as &SyncDynMigration,
        &RollbackApp1003 as &SyncDynMigration,
    ])
    .unwrap();

    engine.run(&test_db.database()).await.unwrap();

    insta::assert_snapshot!(
        dry_run_snapshot(
            &engine,
            &test_db.database(),
            &mut output,
            "0001",
            "rollback_app1"
        )
        .await
    );
}

#[cot_macros::dbtest]
async fn test_migration_rollback_unrelated_apps(test_db: &mut TestDatabase) {
    let mut output = Vec::new();
    let mut migrations = DatabaseUserApp::new().migrations();

    #[expect(trivial_casts)]
    migrations.extend(wrap_migrations(&[
        &RollbackApp1Initial as &SyncDynMigration,
        &RollbackApp10002 as &SyncDynMigration,
        &RollbackApp2Initial as &SyncDynMigration,
    ]));
    migrations.extend(SessionApp::new().migrations());
    let engine = MigrationEngine::new(migrations).unwrap();
    engine.run(&test_db.database()).await.unwrap();

    insta::assert_snapshot!(
        dry_run_snapshot(
            &engine,
            &test_db.database(),
            &mut output,
            "0001",
            "rollback_app1"
        )
        .await
    );
}

#[cot_macros::dbtest]
async fn test_migration_engine_rollback_includes_dependent_apps(test_db: &mut TestDatabase) {
    let mut output = Vec::new();
    #[expect(trivial_casts)]
    let engine = MigrationEngine::new([
        &RollbackApp1Initial as &SyncDynMigration,
        &RollbackApp10002 as &SyncDynMigration,
        &RollbackDependentInitial as &SyncDynMigration,
        &RollbackApp2Initial as &SyncDynMigration,
    ])
    .unwrap();
    engine.run(&test_db.database()).await.unwrap();

    insta::assert_snapshot!(
        dry_run_snapshot(
            &engine,
            &test_db.database(),
            &mut output,
            "0001",
            "rollback_app1"
        )
        .await
    );
}

#[cot_macros::dbtest]
async fn test_migration_engine_rollback_zero(test_db: &mut TestDatabase) {
    let mut output = Vec::new();
    #[expect(trivial_casts)]
    let engine = MigrationEngine::new([
        &RollbackApp1Initial as &SyncDynMigration,
        &RollbackApp10002 as &SyncDynMigration,
        &RollbackApp1003 as &SyncDynMigration,
        &RollbackApp2Initial as &SyncDynMigration,
    ])
    .unwrap();
    engine.run(&test_db.database()).await.unwrap();

    insta::assert_snapshot!(
        dry_run_snapshot(
            &engine,
            &test_db.database(),
            &mut output,
            "zero",
            "rollback_app1"
        )
        .await
    );
}
