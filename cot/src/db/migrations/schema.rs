//! Machine-readable description of a project's database schema.
//!
//! This is used by the Cot CLI to generate migrations: the project binary
//! dumps both the schema of its current models and the schema obtained by
//! replaying all its migrations, and the CLI generates operations from the
//! difference between the two.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::db::migrations::{
    DynMigration, Field, MigrationEngine, OperationInner, SyncDynMigration,
};
use crate::db::{
    ColumnType, ForeignKeyOnDeletePolicy, ForeignKeyOnUpdatePolicy, Identifier, Model,
};

/// The current version of the [`ProjectSchema`] JSON schema.
pub const SCHEMA_VERSION: u32 = 1;

/// Hidden subcommand used to ask a Cot project binary to print its
/// [`ProjectSchema`] as JSON.
pub const SCHEMA_SUBCOMMAND: &str = "cot-internal-schema";

/// All the application models compiled into the binary.
///
/// Populated automatically by the `#[model]` macro, which registers every
/// application model.
#[doc(hidden)]
#[linkme::distributed_slice]
pub static MODELS: [ModelSchema];

/// The schema of a single model, as registered in [`MODELS`].
#[doc(hidden)]
#[derive(Debug, Copy, Clone)]
pub struct ModelSchema {
    app_name: &'static str,
    table_name: Identifier,
    fields: &'static [Field],
}

impl ModelSchema {
    /// Returns the schema of the given model.
    #[must_use]
    pub const fn of<T: Model>() -> Self {
        Self {
            app_name: T::APP_NAME,
            table_name: T::TABLE_NAME,
            fields: T::MIGRATION_FIELDS,
        }
    }
}

/// The schema of a whole project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSchema {
    /// Schema version this structure was serialized with.
    pub version: u32,
    /// The tables of all the application models compiled into the binary.
    pub models: Vec<TableState>,
    /// The tables as they exist after applying all the project's migrations.
    pub migrated_models: Vec<TableState>,
    /// All the project's migrations, in the order they are applied.
    pub migrations: Vec<MigrationRef>,
}

impl ProjectSchema {
    /// Builds the project schema from the given models and migrations.
    ///
    /// # Errors
    ///
    /// Returns an error if the migrations cannot be sorted (e.g. there is a
    /// dependency cycle).
    pub fn new(
        models: &[ModelSchema],
        migrations: Vec<Box<SyncDynMigration>>,
    ) -> crate::db::Result<Self> {
        let mut models: Vec<_> = models
            .iter()
            .map(|model| TableState {
                app: model.app_name.to_owned(),
                table_name: model.table_name.to_string(),
                fields: model.fields.iter().map(FieldState::from).collect(),
            })
            .collect();
        models.sort_by(|a, b| a.table_name.cmp(&b.table_name));

        let engine = MigrationEngine::new(migrations)?;
        let mut tables: BTreeMap<String, TableState> = BTreeMap::new();
        for migration in engine.migrations() {
            for operation in migration.operations() {
                apply_operation(&mut tables, migration.app_name(), &operation.inner);
            }
        }

        Ok(Self {
            version: SCHEMA_VERSION,
            models,
            migrated_models: tables.into_values().collect(),
            migrations: engine
                .migrations()
                .iter()
                .map(|migration| MigrationRef {
                    app: migration.app_name().to_owned(),
                    name: migration.name().to_owned(),
                })
                .collect(),
        })
    }
}

fn apply_operation(
    tables: &mut BTreeMap<String, TableState>,
    app: &str,
    operation: &OperationInner,
) {
    match operation {
        OperationInner::CreateModel {
            table_name, fields, ..
        } => {
            tables.insert(
                table_name.to_string(),
                TableState {
                    app: app.to_owned(),
                    table_name: table_name.to_string(),
                    fields: fields.iter().map(FieldState::from).collect(),
                },
            );
        }
        OperationInner::AddField { table_name, field } => {
            if let Some(table) = tables.get_mut(table_name.as_str()) {
                table.fields.push(FieldState::from(field));
            }
        }
        OperationInner::RemoveField { table_name, field } => {
            if let Some(table) = tables.get_mut(table_name.as_str()) {
                table.fields.retain(|f| f.name != field.name.as_str());
            }
        }
        OperationInner::RemoveModel { table_name, .. } => {
            tables.remove(table_name.as_str());
        }
        // custom operations don't change the schema
        OperationInner::Custom { .. } => {}
    }
}

/// A reference to a migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationRef {
    /// The name of the app the migration belongs to.
    pub app: String,
    /// The name of the migration.
    pub name: String,
}

/// The state of a single database table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableState {
    /// The name of the app the table belongs to.
    pub app: String,
    /// The name of the table.
    pub table_name: String,
    /// The columns of the table.
    pub fields: Vec<FieldState>,
}

/// The state of a single database column.
#[expect(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldState {
    /// The name of the column.
    pub name: String,
    /// The type of the column.
    pub ty: ColumnType,
    /// Whether the column is a primary key.
    pub primary_key: bool,
    /// Whether the column is an auto-incrementing value.
    pub auto_value: bool,
    /// Whether the column can be null.
    pub null: bool,
    /// Whether the column has a unique constraint.
    pub unique: bool,
    /// The foreign key this column references, if any.
    pub foreign_key: Option<ForeignKeyState>,
}

impl From<&Field> for FieldState {
    fn from(field: &Field) -> Self {
        Self {
            name: field.name.to_string(),
            ty: field.ty,
            primary_key: field.primary_key,
            auto_value: field.auto_value,
            null: field.null,
            unique: field.unique,
            foreign_key: field.foreign_key.map(|fk| ForeignKeyState {
                table_name: fk.model.to_string(),
                field: fk.field.to_string(),
                on_delete: fk.on_delete,
                on_update: fk.on_update,
            }),
        }
    }
}

/// The state of a foreign key constraint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignKeyState {
    /// The name of the referenced table.
    pub table_name: String,
    /// The name of the referenced column.
    pub field: String,
    /// What happens when the referenced row is deleted.
    pub on_delete: ForeignKeyOnDeletePolicy,
    /// What happens when the referenced row is updated.
    pub on_update: ForeignKeyOnUpdatePolicy,
}

impl ColumnType {
    /// Returns the path of the canonical Rust type that is stored in a column
    /// of this type, as used in the generated migration files (which import
    /// `cot::db`).
    ///
    /// This deliberately returns a type that is not necessarily the same as the type
    /// used in the model definition, so that migration models do not depend on any
    /// user-defined types.
    #[doc(hidden)]
    #[must_use]
    pub fn canonical_rust_type(self) -> String {
        match self {
            Self::Boolean => "bool".to_owned(),
            Self::TinyInteger => "i8".to_owned(),
            Self::SmallInteger => "i16".to_owned(),
            Self::Integer => "i32".to_owned(),
            Self::BigInteger => "i64".to_owned(),
            Self::TinyUnsignedInteger => "u8".to_owned(),
            Self::SmallUnsignedInteger => "u16".to_owned(),
            Self::UnsignedInteger => "u32".to_owned(),
            Self::BigUnsignedInteger => "u64".to_owned(),
            Self::Float => "f32".to_owned(),
            Self::Double => "f64".to_owned(),
            Self::Time => "chrono::NaiveTime".to_owned(),
            Self::Date => "chrono::NaiveDate".to_owned(),
            Self::DateTime => "chrono::NaiveDateTime".to_owned(),
            Self::DateTimeWithTimeZone => "chrono::DateTime<chrono::FixedOffset>".to_owned(),
            Self::Text => "String".to_owned(),
            Self::Blob => "Vec<u8>".to_owned(),
            Self::String(limit) => format!("db::LimitedString<{limit}>"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::DatabaseField;
    use crate::db::migrations::{Migration, MigrationDependency, Operation};

    #[test]
    fn canonical_rust_type_round_trip() {
        macro_rules! check {
            ($($ty:ty),* $(,)?) => {$(
                let column_type = <$ty as DatabaseField>::TYPE;
                assert_eq!(
                    column_type.canonical_rust_type().replace(' ', ""),
                    stringify!($ty).replace(' ', ""),
                );
            )*};
        }

        check!(
            bool,
            i8,
            i16,
            i32,
            i64,
            u8,
            u16,
            u32,
            u64,
            f32,
            f64,
            chrono::NaiveTime,
            chrono::NaiveDate,
            chrono::NaiveDateTime,
            chrono::DateTime<chrono::FixedOffset>,
            String,
            Vec<u8>,
            db::LimitedString<42>,
        );
    }

    struct M1;
    impl Migration for M1 {
        const APP_NAME: &'static str = "app";
        const MIGRATION_NAME: &'static str = "m_0001_initial";
        const DEPENDENCIES: &'static [MigrationDependency] = &[];
        const OPERATIONS: &'static [Operation] = &[
            Operation::create_model()
                .table_name(Identifier::new("app__a"))
                .fields(&[
                    Field::new(Identifier::new("id"), ColumnType::Integer)
                        .primary_key()
                        .auto(),
                    Field::new(Identifier::new("old"), ColumnType::Text),
                ])
                .build(),
            Operation::create_model()
                .table_name(Identifier::new("app__b"))
                .fields(&[Field::new(Identifier::new("id"), ColumnType::Integer).primary_key()])
                .build(),
        ];
    }

    struct M2;
    impl Migration for M2 {
        const APP_NAME: &'static str = "app";
        const MIGRATION_NAME: &'static str = "m_0002_auto";
        const DEPENDENCIES: &'static [MigrationDependency] =
            &[MigrationDependency::migration("app", "m_0001_initial")];
        const OPERATIONS: &'static [Operation] = &[
            Operation::remove_field()
                .table_name(Identifier::new("app__a"))
                .field(Field::new(Identifier::new("old"), ColumnType::Text))
                .build(),
            Operation::add_field()
                .table_name(Identifier::new("app__a"))
                .field(Field::new(Identifier::new("new"), ColumnType::String(10)).null())
                .build(),
            Operation::remove_model()
                .table_name(Identifier::new("app__b"))
                .fields(&[])
                .build(),
        ];
    }

    #[test]
    fn replay_migrations() {
        // passed in reverse order to check that migrations get sorted
        let schema = ProjectSchema::new(&[], vec![Box::new(M2), Box::new(M1)]).unwrap();

        assert_eq!(
            schema.migrations,
            vec![
                MigrationRef {
                    app: "app".to_owned(),
                    name: "m_0001_initial".to_owned(),
                },
                MigrationRef {
                    app: "app".to_owned(),
                    name: "m_0002_auto".to_owned(),
                },
            ]
        );
        assert_eq!(schema.migrated_models.len(), 1);
        let table = &schema.migrated_models[0];
        assert_eq!(table.table_name, "app__a");
        let fields: Vec<_> = table
            .fields
            .iter()
            .map(|f| (f.name.as_str(), f.ty, f.null))
            .collect();
        assert_eq!(
            fields,
            vec![
                ("id", ColumnType::Integer, false),
                ("new", ColumnType::String(10), true),
            ]
        );
    }
}
