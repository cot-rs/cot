use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use cot_cli::migration_generator::{
    self, DynDependency, DynOperation, MigrationAsSource, MigrationGenerator,
    MigrationGeneratorOptions, SourceFile,
};
use cot_cli::test_utils;
use syn::parse_quote;

pub const EXAMPLE_DATABASE_MODEL: &str = include_str!("resources/example_database_model.rs");

/// Test that the migration generator can generate a "create model" migration
/// for a given model that has an expected state.
#[test]
fn create_model_state_test() {
    let generator = test_generator();
    let src = include_str!("migration_generator/create_model.rs");
    let source_files = vec![SourceFile::parse(PathBuf::from("main.rs"), src).unwrap()];

    let migration = generator
        .generate_migrations_as_generated_from_files(source_files)
        .unwrap()
        .unwrap();

    assert_eq!(migration.migration_name, "m_0001_initial");
    assert!(migration.dependencies.is_empty());

    let (table_name, fields) = unwrap_create_model(&migration.operations[0]);
    assert_eq!(table_name, "parent");
    assert_eq!(fields.len(), 1);

    let (table_name, fields) = unwrap_create_model(&migration.operations[1]);
    assert_eq!(table_name, "my_model");
    assert_eq!(fields.len(), 4);

    let field = &fields[0];
    assert_eq!(field.column_name, "id");
    assert!(field.primary_key);
    assert!(field.auto_value);
    assert!(field.foreign_key.clone().is_none());

    let field = &fields[1];
    assert_eq!(field.column_name, "field_1");
    assert!(!field.primary_key);
    assert!(!field.auto_value);
    assert!(field.foreign_key.clone().is_none());

    let field = &fields[2];
    assert_eq!(field.column_name, "field_2");
    assert!(!field.primary_key);
    assert!(!field.auto_value);
    assert!(field.foreign_key.clone().is_none());

    let field = &fields[3];
    assert_eq!(field.column_name, "parent");
    assert!(!field.primary_key);
    assert!(!field.auto_value);
    assert!(field.foreign_key.clone().is_some());
}

#[test]
fn create_models_foreign_key() {
    let generator = test_generator();
    let src = include_str!("migration_generator/foreign_key.rs");
    let source_files = vec![SourceFile::parse(PathBuf::from("main.rs"), src).unwrap()];

    let migration = generator
        .generate_migrations_as_generated_from_files(source_files)
        .unwrap()
        .unwrap();

    assert_eq!(migration.dependencies.len(), 0);
    assert_eq!(migration.operations.len(), 2);

    // Parent must be created before Child
    let (table_name, fields) = unwrap_create_model(&migration.operations[0]);
    assert_eq!(table_name, "parent");
    assert_eq!(fields.len(), 1);

    let (table_name, fields) = unwrap_create_model(&migration.operations[1]);
    assert_eq!(table_name, "child");
    assert_eq!(fields.len(), 2);

    let field = &fields[0];
    assert_eq!(field.column_name, "id");
    assert!(field.primary_key);
    assert!(field.auto_value);
    assert!(field.foreign_key.clone().is_none());

    let field = &fields[1];
    assert_eq!(field.column_name, "parent");
    assert!(!field.primary_key);
    assert!(!field.auto_value);
    assert!(field.foreign_key.clone().is_some());
}

#[test]
fn create_models_foreign_key_cycle() {
    let generator = test_generator();
    let src = include_str!("migration_generator/foreign_key_cycle.rs");
    let source_files = vec![SourceFile::parse(PathBuf::from("main.rs"), src).unwrap()];

    let migration = generator
        .generate_migrations_as_generated_from_files(source_files)
        .unwrap()
        .unwrap();

    assert_eq!(migration.dependencies.len(), 0);
    assert_eq!(migration.operations.len(), 3);

    // Parent must be created before Child
    let (table_name, fields) = unwrap_create_model(&migration.operations[0]);
    assert_eq!(table_name, "parent");
    assert_eq!(fields.len(), 1);

    let (table_name, fields) = unwrap_create_model(&migration.operations[1]);
    assert_eq!(table_name, "child");
    assert_eq!(fields.len(), 2);

    let (table_name, field) = unwrap_add_field(&migration.operations[2]);
    assert_eq!(table_name, "parent");
    assert_eq!(field.field_name, "child");
}

#[test]
fn create_models_foreign_key_two_migrations() {
    let generator = test_generator();

    let src = include_str!("migration_generator/foreign_key_two_migrations/step_1.rs");
    let source_files = vec![SourceFile::parse(PathBuf::from("main.rs"), src).unwrap()];
    let migration_file = generator
        .generate_migrations_as_source_from_files(source_files)
        .unwrap()
        .unwrap();

    let src = include_str!("migration_generator/foreign_key_two_migrations/step_2.rs");
    let source_files = vec![
        SourceFile::parse(PathBuf::from("main.rs"), src).unwrap(),
        SourceFile::parse(PathBuf::from(&migration_file.name), &migration_file.content).unwrap(),
    ];
    let migration = generator
        .generate_migrations_as_generated_from_files(source_files)
        .unwrap()
        .unwrap();

    assert_eq!(migration.dependencies.len(), 2);
    assert!(migration.dependencies.contains(&DynDependency::Migration {
        app: "my_crate".to_string(),
        migration: "m_0001_initial".to_string()
    }));
    assert!(migration.dependencies.contains(&DynDependency::Model {
        model_type: parse_quote!(crate::Parent),
    }));

    assert_eq!(migration.operations.len(), 1);

    let (table_name, _fields) = unwrap_create_model(&migration.operations[0]);
    assert_eq!(table_name, "child");
}

/// Test that the migration generator can generate a "create model" migration
/// for a given model which compiles successfully.
#[test]
#[cfg_attr(miri, ignore)] // unsupported operation: extern static `pidfd_spawnp` is not supported by Miri
fn create_model_compile_test() {
    let generator = test_generator();
    let src = include_str!("migration_generator/create_model.rs");
    let source_files = vec![SourceFile::parse(PathBuf::from("main.rs"), src).unwrap()];

    let migration_opt = generator
        .generate_migrations_as_source_from_files(source_files)
        .unwrap()
        .unwrap();
    let MigrationAsSource {
        name: migration_name,
        content: migration_content,
    } = migration_opt;

    let source_with_migrations = format!(
        r"
{src}

mod migrations {{
    mod {migration_name} {{
        {migration_content}
    }}
}}"
    );

    let temp_dir = tempfile::tempdir().unwrap();
    let test_path = temp_dir.path().join("main.rs");
    std::fs::write(&test_path, source_with_migrations).unwrap();

    let t = trybuild::TestCases::new();
    t.pass(&test_path);
}

#[test]
fn write_migrations_module() {
    let tempdir = tempfile::tempdir().unwrap();

    let generator = MigrationGenerator::new(
        PathBuf::from("Cargo.toml"),
        String::from("my_crate"),
        MigrationGeneratorOptions {
            app_name: Some("my_crate".to_string()),
            output_dir: Some(tempdir.path().to_path_buf()),
        },
    );

    let migrations_dir = tempdir.path().join("migrations");
    std::fs::create_dir(&migrations_dir).unwrap();

    File::create(migrations_dir.join("m_0001_initial.rs")).unwrap();
    File::create(migrations_dir.join("m_0002_auto.rs")).unwrap();

    generator.write_migrations_module().unwrap();

    let migrations_file = tempdir.path().join("migrations.rs");
    assert!(migrations_file.exists());

    let content = std::fs::read_to_string(&migrations_file).unwrap();
    assert!(content.contains("pub const MIGRATIONS"));
    assert!(content.contains("pub mod m_0001_initial;"));
    assert!(content.contains("&m_0001_initial::Migration"));
    assert!(content.contains("pub mod m_0002_auto;"));
    assert!(content.contains("&m_0002_auto::Migration"));
}

#[test]
fn find_source_files() {
    let tempdir = tempfile::tempdir().unwrap();
    let nested = tempdir.path().join("nested");
    std::fs::create_dir(&nested).unwrap();

    let file_name = "main.rs";
    let nested_file_name = "nested.rs";
    File::create(tempdir.path().join(file_name)).unwrap();
    File::create(tempdir.path().join(nested_file_name)).unwrap();

    let source_files = MigrationGenerator::find_source_files(tempdir.path()).unwrap();
    assert_eq!(source_files.len(), 2);
    assert!(
        source_files
            .iter()
            .any(|f| f.file_name().unwrap() == file_name)
    );
    assert!(
        source_files
            .iter()
            .any(|f| f.file_name().unwrap() == nested_file_name)
    );
}

#[test]
#[cfg_attr(miri, ignore)] // unsupported operation: extern static `pidfd_spawnp` is not supported by Miri
fn list_migrations() {
    let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
    let package_name = temp_dir.path().file_name().unwrap().to_str().unwrap();
    test_utils::make_package(temp_dir.path()).unwrap();
    let mut main = std::fs::OpenOptions::new()
        .append(true)
        .open(temp_dir.path().join("src").join("main.rs"))
        .unwrap();
    write!(main, "{EXAMPLE_DATABASE_MODEL}").unwrap();
    migration_generator::make_migrations(
        temp_dir.path(),
        MigrationGeneratorOptions {
            app_name: None,
            output_dir: None,
        },
    )
    .unwrap();

    let migrations = migration_generator::list_migrations(temp_dir.path()).unwrap();

    assert_eq!(migrations.len(), 1);
    assert!(migrations.contains_key(package_name));
    assert_eq!(migrations.get(package_name).unwrap()[0], "m_0001_initial");
}

#[test]
fn list_migrations_missing_cargo_toml() {
    let tmp_dir = tempfile::tempdir().unwrap();

    let migrations = migration_generator::list_migrations(tmp_dir.path());

    assert!(migrations.is_err());
}

#[test]
#[cfg_attr(miri, ignore)] // unsupported operation: extern static `pidfd_spawnp` is not supported by Miri
fn list_migrations_missing_migrations_dir() {
    let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
    test_utils::make_package(temp_dir.path()).unwrap();

    let migrations = migration_generator::list_migrations(temp_dir.path()).unwrap();

    assert!(migrations.is_empty());
}

fn test_generator() -> MigrationGenerator {
    MigrationGenerator::new(
        PathBuf::from("Cargo.toml"),
        String::from("my_crate"),
        MigrationGeneratorOptions::default(),
    )
}

fn unwrap_create_model(op: &DynOperation) -> (&str, Vec<cot_codegen::model::Field>) {
    if let DynOperation::CreateModel {
        table_name, fields, ..
    } = op
    {
        (table_name, fields.clone())
    } else {
        panic!("expected create model operation");
    }
}

fn unwrap_add_field(op: &DynOperation) -> (&str, cot_codegen::model::Field) {
    if let DynOperation::AddField {
        table_name, field, ..
    } = op
    {
        (table_name, *field.clone())
    } else {
        panic!("expected create model operation");
    }
}
