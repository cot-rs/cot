//! Migration generator based on the database schema reported by the compiled
//! project binary.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use cot::db::migrations::schema::{FieldState, MigrationRef, ProjectSchema, TableState};
use cot::db::{ForeignKeyOnDeletePolicy, ForeignKeyOnUpdatePolicy};
use cot::utils::cli::{StatusType, print_status_msg};
use heck::{ToSnakeCase, ToUpperCamelCase};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::project;
use crate::utils::{CargoTomlManager, PackageManager};

const MIGRATIONS_MODULE_NAME: &str = "migrations";
const MIGRATIONS_MODULE_PREFIX: &str = "m_";

#[derive(Debug, Clone, Default)]
pub struct MigrationGeneratorOptions {
    pub app_name: Option<String>,
    pub output_dir: Option<PathBuf>,
}

pub fn make_migrations(path: &Path, options: MigrationGeneratorOptions) -> anyhow::Result<()> {
    with_package(path, |package| {
        let generator = MigrationGenerator::load(package, options)?;
        let Some(migration) = generator.generate_auto_migration()? else {
            print_status_msg(
                StatusType::Notice,
                "No changes in models detected; no migrations were generated",
            );
            return Ok(());
        };
        generator.write(&migration)
    })
}

pub fn create_new_migration(
    path: &Path,
    name: &str,
    options: MigrationGeneratorOptions,
) -> anyhow::Result<()> {
    with_package(path, |package| {
        let generator = MigrationGenerator::load(package, options)?;
        let migration = generator.generate_custom_migration(name)?;
        generator.write(&migration)
    })
}

fn with_package(
    path: &Path,
    f: impl FnOnce(&PackageManager) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let Some(manager) = CargoTomlManager::from_path(path)? else {
        bail!("no Cargo.toml found in the specified directory or any parent directory")
    };
    match manager {
        CargoTomlManager::Workspace(workspace) => {
            f(workspace.get_current_package_manager().context(
                "generating migrations for workspaces is not supported yet. \
                 Please generate migrations for each package separately",
            )?)
        }
        CargoTomlManager::Package(package) => f(&package),
    }
}

#[derive(Debug, Clone)]
struct MigrationAsSource {
    name: String,
    content: String,
}

#[derive(Debug)]
struct MigrationGenerator {
    app_name: String,
    /// The directory to write `migrations.rs` and `migrations/` to.
    src_path: PathBuf,
    schema: ProjectSchema,
}

impl MigrationGenerator {
    fn load(package: &PackageManager, options: MigrationGeneratorOptions) -> anyhow::Result<Self> {
        let schema = project::load_schema(package.get_package_path(), package.get_package_name())?;
        Ok(Self {
            app_name: options
                .app_name
                .unwrap_or_else(|| package.get_package_name().to_owned()),
            src_path: options
                .output_dir
                .unwrap_or_else(|| package.get_package_path().join("src")),
            schema,
        })
    }

    fn write(&self, migration: &MigrationAsSource) -> anyhow::Result<()> {
        print_status_msg(
            StatusType::Creating,
            &format!("Migration '{}'", migration.name),
        );

        let migrations_dir = self.src_path.join(MIGRATIONS_MODULE_NAME);
        std::fs::create_dir_all(&migrations_dir).with_context(|| {
            format!(
                "unable to create migrations directory: {}",
                migrations_dir.display()
            )
        })?;
        let migration_file = migrations_dir.join(format!("{}.rs", migration.name));
        print_status_msg(
            StatusType::Creating,
            &format!("Migration file '{}'", migration_file.display()),
        );
        std::fs::write(&migration_file, &migration.content).with_context(|| {
            format!(
                "unable to write migration file: {}",
                migration_file.display()
            )
        })?;
        print_status_msg(
            StatusType::Created,
            &format!("Migration file '{}'", migration_file.display()),
        );

        self.write_migrations_module()
            .context("unable to write migrations.rs")?;

        print_status_msg(
            StatusType::Created,
            &format!("Migration '{}'", migration.name),
        );
        Ok(())
    }

    fn write_migrations_module(&self) -> anyhow::Result<()> {
        let migrations = migration_files(&self.src_path.join(MIGRATIONS_MODULE_NAME))?;
        let mods = migrations.iter().map(|m| format_ident!("{m}"));
        let refs = mods.clone();
        let contents = format_tokens(quote! {
            #(pub mod #mods;)*

            pub const MIGRATIONS: &[&cot::db::migrations::SyncDynMigration] = &[
                #(&#refs::Migration),*
            ];
        });

        let header = migration_header();
        std::fs::write(
            self.src_path.join(format!("{MIGRATIONS_MODULE_NAME}.rs")),
            format!("//! List of migrations for the current app.\n//!\n{header}\n\n{contents}"),
        )?;
        Ok(())
    }

    fn app_tables<'a>(&self, tables: &'a [TableState]) -> Vec<&'a TableState> {
        tables.iter().filter(|t| t.app == self.app_name).collect()
    }

    fn app_migrations(&self) -> Vec<&MigrationRef> {
        self.schema
            .migrations
            .iter()
            .filter(|m| m.app == self.app_name)
            .collect()
    }

    fn generate_auto_migration(&self) -> anyhow::Result<Option<MigrationAsSource>> {
        let current = self.app_tables(&self.schema.models);
        let previous = self.app_tables(&self.schema.migrated_models);
        let operations = generate_operations(&current, &previous)?;
        if operations.is_empty() {
            return Ok(None);
        }

        let migrations = self.app_migrations();
        let name = if migrations.is_empty() {
            format!("{MIGRATIONS_MODULE_PREFIX}0001_initial")
        } else {
            let date_time = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            format!(
                "{MIGRATIONS_MODULE_PREFIX}{:04}_auto_{date_time}",
                next_migration_number(&migrations)?
            )
        };

        let mut dependencies = self.base_dependencies();
        dependencies.extend(self.foreign_key_dependencies(&operations)?);
        let operations: Vec<_> = operations.iter().map(Operation::repr).collect();

        let migration = self.migration_def(&name, &dependencies, &quote!(#(#operations,)*));
        let models = self.frozen_models(&current)?;
        Ok(Some(MigrationAsSource {
            content: migration_file_content(migration, models),
            name,
        }))
    }

    fn generate_custom_migration(&self, suffix: &str) -> anyhow::Result<MigrationAsSource> {
        let migrations = self.app_migrations();
        let name = format!(
            "{MIGRATIONS_MODULE_PREFIX}{:04}_{suffix}",
            next_migration_number(&migrations)?
        );

        let operations = quote! {
            db::migrations::Operation::custom(forwards).backwards(backwards).build(),
        };
        let dependencies = self.base_dependencies();
        let mut migration = self.migration_def(&name, &dependencies, &operations);
        migration.extend(quote! {
            #[db::migrations::migration_op]
            async fn forwards(_ctx: db::migrations::MigrationContext<'_>) -> db::Result<()> {
                Ok(())
            }

            #[db::migrations::migration_op]
            async fn backwards(_ctx: db::migrations::MigrationContext<'_>) -> db::Result<()> {
                Err(db::DatabaseError::MigrationError(
                    db::migrations::MigrationEngineError::Custom("Backwards migration not implemented".into())
                ))
            }
        });

        // models as they are when this migration is applied, i.e. after all the
        // previous migrations
        let tables = self.app_tables(&self.schema.migrated_models);
        let models = self.frozen_models(&tables)?;
        Ok(MigrationAsSource {
            content: migration_file_content(migration, models),
            name,
        })
    }

    fn migration_def(
        &self,
        name: &str,
        dependencies: &[TokenStream],
        operations: &TokenStream,
    ) -> TokenStream {
        let app_name = &self.app_name;
        quote! {
            #[derive(Debug, Copy, Clone)]
            pub(super) struct Migration;

            impl db::migrations::Migration for Migration {
                const APP_NAME: &'static str = #app_name;
                const MIGRATION_NAME: &'static str = #name;
                const DEPENDENCIES: &'static [db::migrations::MigrationDependency] = &[
                    #(#dependencies,)*
                ];
                const OPERATIONS: &'static [db::migrations::Operation] = &[
                    #operations
                ];
            }
        }
    }

    fn base_dependencies(&self) -> Vec<TokenStream> {
        self.app_migrations()
            .last()
            .map(|MigrationRef { app, name }| {
                quote!(db::migrations::MigrationDependency::migration(#app, #name))
            })
            .into_iter()
            .collect()
    }

    /// Dependencies on the models from other apps referenced by foreign keys.
    fn foreign_key_dependencies(
        &self,
        operations: &[Operation],
    ) -> anyhow::Result<Vec<TokenStream>> {
        let mut targets = BTreeSet::new();
        for op in operations {
            let fields = match op {
                Operation::CreateModel { fields, .. } => fields.as_slice(),
                Operation::AddField { field, .. } => std::slice::from_ref(field),
                Operation::RemoveField { .. } | Operation::RemoveModel { .. } => &[],
            };
            for fk in fields.iter().filter_map(|f| f.foreign_key.as_ref()) {
                let table = self.find_table(&fk.table_name)?;
                if table.app != self.app_name {
                    targets.insert((table.app.as_str(), table.table_name.as_str()));
                }
            }
        }

        Ok(targets
            .into_iter()
            .map(|(app, table)| {
                quote! {
                    db::migrations::MigrationDependency::model(
                        #app,
                        db::Identifier::new(#table),
                    )
                }
            })
            .collect())
    }

    /// Finds a table either in the current models of this app, or in the
    /// already migrated tables of any app.
    fn find_table(&self, table_name: &str) -> anyhow::Result<&TableState> {
        self.schema
            .models
            .iter()
            .filter(|t| t.app == self.app_name)
            .chain(&self.schema.migrated_models)
            .find(|t| t.table_name == table_name)
            .with_context(|| {
                format!(
                    "table `{table_name}` referenced by a foreign key was not found; if it \
                     belongs to another app, generate migrations for that app first"
                )
            })
    }

    /// Generates migration models for given tables of this app, plus
    /// primary-key-only models for the tables in other apps they refer to.
    fn frozen_models(&self, tables: &[&TableState]) -> anyhow::Result<TokenStream> {
        let mut external = BTreeMap::new();
        for fk in tables
            .iter()
            .flat_map(|t| &t.fields)
            .filter_map(|f| f.foreign_key.as_ref())
        {
            let table = self.find_table(&fk.table_name)?;
            if !tables.iter().any(|t| t.table_name == table.table_name) {
                external.insert(table.table_name.as_str(), table);
            }
        }

        let mut models = Vec::new();
        for table in tables {
            models.push(self.frozen_model(table, &table.fields)?);
        }
        for table in external.into_values() {
            let pk: Vec<_> = table
                .fields
                .iter()
                .filter(|f| f.primary_key)
                .cloned()
                .collect();
            models.push(self.frozen_model(table, &pk)?);
        }
        Ok(quote!(#(#models)*))
    }

    fn frozen_model(
        &self,
        table: &TableState,
        fields: &[FieldState],
    ) -> anyhow::Result<TokenStream> {
        let name = self.frozen_model_name(&table.table_name)?;
        let short_name = strip_app_prefix(table)?;

        let app_name_arg = (table.app != self.app_name).then(|| {
            let app = &table.app;
            quote!(, app_name = #app)
        });

        let fields = fields
            .iter()
            .map(|field| self.frozen_field(field))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(quote! {
            #[derive(Debug)]
            #[db::model(model_type = "migration", table_name = #short_name #app_name_arg)]
            struct #name {
                #(#fields,)*
            }
        })
    }

    fn frozen_model_name(&self, table_name: &str) -> anyhow::Result<syn::Ident> {
        let table = self.find_table(table_name)?;
        let short_name = strip_app_prefix(table)?;
        let name = if table.app == self.app_name {
            short_name.to_upper_camel_case()
        } else {
            format!("{}_{short_name}", table.app).to_upper_camel_case()
        };
        Ok(format_ident!("_{name}"))
    }

    fn frozen_field(&self, field: &FieldState) -> anyhow::Result<TokenStream> {
        let name = field_ident(&field.name)?;

        let mut ty: syn::Type = if let Some(fk) = &field.foreign_key {
            let target = self.frozen_model_name(&fk.table_name)?;
            syn::parse_quote!(db::ForeignKey<#target>)
        } else {
            let base: syn::Type = syn::parse_str(&field.ty.canonical_rust_type())
                .expect("canonical Rust type should be valid");
            if field.auto_value {
                syn::parse_quote!(db::Auto<#base>)
            } else {
                base
            }
        };
        if field.null {
            ty = syn::parse_quote!(Option<#ty>);
        }

        let mut attrs = Vec::new();
        if field.primary_key {
            attrs.push(quote!(primary_key));
        }
        if field.unique {
            attrs.push(quote!(unique));
        }
        if let Some(fk) = &field.foreign_key {
            let mut args = Vec::new();
            if fk.on_delete != ForeignKeyOnDeletePolicy::default() {
                let policy = format!("{:?}", fk.on_delete).to_snake_case();
                args.push(quote!(on_delete = #policy));
            }
            if fk.on_update != ForeignKeyOnUpdatePolicy::default() {
                let policy = format!("{:?}", fk.on_update).to_snake_case();
                args.push(quote!(on_update = #policy));
            }
            if !args.is_empty() {
                attrs.push(quote!(foreign_key(#(#args),*)));
            }
        }
        let attrs = (!attrs.is_empty()).then(|| quote!(#[model(#(#attrs),*)]));

        Ok(quote!(#attrs #name: #ty))
    }
}

fn migration_file_content(migration: TokenStream, models: TokenStream) -> String {
    let header = migration_header();
    let migration = format_tokens(migration);
    let models = format_tokens(models);
    format!("{header}\n\nuse cot::db;\n\n{migration}\n{models}")
}

fn migration_header() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let date_time = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%:z");
    format!("//! Generated by cot CLI {version} on {date_time}")
}

fn format_tokens(tokens: TokenStream) -> String {
    let file: syn::File = syn::parse2(tokens).expect("generated code should be valid Rust");
    prettyplease::unparse(&file)
}

/// Returns the names of the migration modules in given directory, sorted.
fn migration_files(migrations_dir: &Path) -> anyhow::Result<Vec<String>> {
    let entries = match std::fs::read_dir(migrations_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).context("unable to read migrations directory"),
    };

    let mut migrations = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.is_file()
            && path.extension() == Some("rs".as_ref())
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            && stem.starts_with(MIGRATIONS_MODULE_PREFIX)
        {
            migrations.push(stem.to_owned());
        }
    }
    migrations.sort();
    Ok(migrations)
}

fn next_migration_number(migrations: &[&MigrationRef]) -> anyhow::Result<u32> {
    let Some(last) = migrations.last() else {
        return Ok(1);
    };
    let number = last
        .name
        .split('_')
        .nth(1)
        .and_then(|n| n.parse::<u32>().ok())
        .with_context(|| format!("unable to parse the migration number: {}", last.name))?;
    Ok(number + 1)
}

/// Returns the table name without the `{app}__` prefix, as passed to the
/// `table_name` argument of the `#[model]` attribute.
fn strip_app_prefix(table: &TableState) -> anyhow::Result<&str> {
    let prefix = format!("{}__", table.app.to_snake_case());
    let short_name = table.table_name.strip_prefix(&prefix);
    match short_name {
        // the model macro converts the table name to snake case
        Some(short_name) if short_name.to_snake_case() == short_name => Ok(short_name),
        _ => bail!(
            "table `{}` can't be represented as a migration model (expected a snake_case name \
             prefixed with `{prefix}`)",
            table.table_name
        ),
    }
}

fn field_ident(name: &str) -> anyhow::Result<syn::Ident> {
    syn::parse_str::<syn::Ident>(name)
        .or_else(|_| syn::parse_str::<syn::Ident>(&format!("r#{name}")))
        .ok()
        .with_context(|| format!("column name `{name}` is not a valid Rust identifier"))
}

#[derive(Debug, Clone)]
enum Operation {
    CreateModel {
        table_name: String,
        fields: Vec<FieldState>,
    },
    AddField {
        table_name: String,
        field: FieldState,
    },
    RemoveField {
        table_name: String,
        field: FieldState,
    },
    RemoveModel {
        table_name: String,
        fields: Vec<FieldState>,
    },
}

impl Operation {
    fn repr(&self) -> TokenStream {
        let (builder, table_name, rest) = match self {
            Self::CreateModel { table_name, fields } => {
                let fields = fields.iter().map(field_repr);
                (
                    quote!(create_model),
                    table_name,
                    quote!(.fields(&[#(#fields,)*])),
                )
            }
            Self::AddField { table_name, field } => {
                let field = field_repr(field);
                (quote!(add_field), table_name, quote!(.field(#field)))
            }
            Self::RemoveField { table_name, field } => {
                let field = field_repr(field);
                (quote!(remove_field), table_name, quote!(.field(#field)))
            }
            Self::RemoveModel { table_name, fields } => {
                let fields = fields.iter().map(field_repr);
                (
                    quote!(remove_model),
                    table_name,
                    quote!(.fields(&[#(#fields,)*])),
                )
            }
        };
        quote! {
            db::migrations::Operation::#builder()
                .table_name(db::Identifier::new(#table_name))
                #rest
                .build()
        }
    }
}

/// Generates a `cot::db::migrations::Field` that only contains literal values.
fn field_repr(field: &FieldState) -> TokenStream {
    let name = &field.name;
    // `ColumnType` derives `Debug`, which prints the variant exactly as it's
    // written in Rust (e.g. `String(255)`)
    let ty: syn::Expr = syn::parse_str(&format!("db::ColumnType::{:?}", field.ty))
        .expect("column type should be a valid expression");

    let mut tokens = quote! {
        db::migrations::Field::new(db::Identifier::new(#name), #ty)
    };
    // must be set before the foreign key, which checks nullability
    if field.null {
        tokens = quote!(#tokens.null());
    }
    if field.auto_value {
        tokens = quote!(#tokens.auto());
    }
    if field.primary_key {
        tokens = quote!(#tokens.primary_key());
    }
    if let Some(fk) = &field.foreign_key {
        let table = &fk.table_name;
        let column = &fk.field;
        let on_delete = format_ident!("{}", format!("{:?}", fk.on_delete));
        let on_update = format_ident!("{}", format!("{:?}", fk.on_update));
        tokens = quote! {
            #tokens.foreign_key(
                db::Identifier::new(#table),
                db::Identifier::new(#column),
                db::ForeignKeyOnDeletePolicy::#on_delete,
                db::ForeignKeyOnUpdatePolicy::#on_update,
            )
        };
    }
    if field.unique {
        tokens = quote!(#tokens.unique());
    }
    tokens
}

fn generate_operations(
    current: &[&TableState],
    previous: &[&TableState],
) -> anyhow::Result<Vec<Operation>> {
    let current: BTreeMap<_, _> = current
        .iter()
        .map(|t| (t.table_name.as_str(), *t))
        .collect();
    let previous: BTreeMap<_, _> = previous
        .iter()
        .map(|t| (t.table_name.as_str(), *t))
        .collect();

    let mut creates = Vec::new();
    let mut adds = Vec::new();
    let mut field_removals = Vec::new();
    let mut model_removals = Vec::new();

    for (&table_name, &table) in &current {
        let Some(&prev) = previous.get(table_name) else {
            print_status_msg(StatusType::Created, &format!("Model '{table_name}'"));
            creates.push((table_name.to_owned(), table.fields.clone()));
            continue;
        };

        for field in &table.fields {
            match prev.fields.iter().find(|f| f.name == field.name) {
                None => {
                    print_status_msg(
                        StatusType::Added,
                        &format!("Field '{}' to Model '{table_name}'", field.name),
                    );
                    adds.push(Operation::AddField {
                        table_name: table_name.to_owned(),
                        field: field.clone(),
                    });
                }
                Some(prev_field) if prev_field != field => bail!(
                    "field `{}` of model `{table_name}` was changed, which is not supported yet",
                    field.name
                ),
                Some(_) => {}
            }
        }
        for prev_field in &prev.fields {
            if !table.fields.iter().any(|f| f.name == prev_field.name) {
                print_status_msg(
                    StatusType::Removed,
                    &format!("Field '{}' from Model '{table_name}'", prev_field.name),
                );
                field_removals.push(Operation::RemoveField {
                    table_name: table_name.to_owned(),
                    field: prev_field.clone(),
                });
            }
        }
    }
    for (&table_name, &prev) in &previous {
        if !current.contains_key(table_name) {
            print_status_msg(StatusType::Removed, &format!("Model '{table_name}'"));
            model_removals.push(Operation::RemoveModel {
                table_name: table_name.to_owned(),
                fields: prev.fields.clone(),
            });
        }
    }

    let (creates, deferred_foreign_keys) = order_creates(creates);
    Ok(creates
        .into_iter()
        .chain(deferred_foreign_keys)
        .chain(adds)
        .chain(field_removals)
        .chain(model_removals)
        .collect())
}

/// Orders model creations so that the models referenced by foreign keys are
/// created first. Foreign keys that form cycles are moved out to separate
/// `AddField` operations, returned as the second element.
fn order_creates(mut creates: Vec<(String, Vec<FieldState>)>) -> (Vec<Operation>, Vec<Operation>) {
    let index: BTreeMap<String, usize> = creates
        .iter()
        .enumerate()
        .map(|(i, (name, _))| (name.clone(), i))
        .collect();

    // edge from the referenced model to the model holding the foreign key
    let build_graph = |creates: &[(String, Vec<FieldState>)]| {
        let mut graph = DiGraph::<(), ()>::new();
        for _ in creates {
            graph.add_node(());
        }
        for (i, (name, fields)) in creates.iter().enumerate() {
            for fk in fields.iter().filter_map(|f| f.foreign_key.as_ref()) {
                if let Some(&target) = index.get(&fk.table_name)
                    && fk.table_name != *name
                {
                    graph.update_edge(NodeIndex::new(target), NodeIndex::new(i), ());
                }
            }
        }
        graph
    };

    let graph = build_graph(&creates);
    let mut deferred = Vec::new();
    for edge in petgraph::algo::feedback_arc_set::greedy_feedback_arc_set(&graph) {
        let target = &creates[edge.source().index()].0.clone();
        let (name, fields) = &mut creates[edge.target().index()];
        let (moved, kept): (Vec<_>, Vec<_>) = std::mem::take(fields).into_iter().partition(|f| {
            f.foreign_key
                .as_ref()
                .is_some_and(|fk| fk.table_name == *target)
        });
        *fields = kept;
        deferred.extend(moved.into_iter().map(|field| Operation::AddField {
            table_name: name.clone(),
            field,
        }));
    }

    let graph = build_graph(&creates);
    let order =
        petgraph::algo::toposort(&graph, None).expect("cycles shouldn't exist after removing them");
    let mut creates: Vec<_> = creates.into_iter().map(Some).collect();
    let creates = order
        .into_iter()
        .map(|node| {
            let (table_name, fields) = creates[node.index()]
                .take()
                .expect("each node is visited once");
            Operation::CreateModel { table_name, fields }
        })
        .collect();
    (creates, deferred)
}

#[cfg(test)]
mod tests {
    use cot::db::ColumnType;
    use cot::db::migrations::schema::ForeignKeyState;

    use super::*;

    fn field(name: &str) -> FieldState {
        FieldState {
            name: name.to_owned(),
            ty: ColumnType::Integer,
            primary_key: false,
            auto_value: false,
            null: false,
            unique: false,
            foreign_key: None,
        }
    }

    fn fk(name: &str, table: &str) -> FieldState {
        FieldState {
            foreign_key: Some(ForeignKeyState {
                table_name: table.to_owned(),
                field: "id".to_owned(),
                on_delete: ForeignKeyOnDeletePolicy::default(),
                on_update: ForeignKeyOnUpdatePolicy::default(),
            }),
            ..field(name)
        }
    }

    fn table(name: &str, fields: Vec<FieldState>) -> TableState {
        TableState {
            app: "app".to_owned(),
            table_name: name.to_owned(),
            fields,
        }
    }

    fn summary(ops: &[Operation]) -> Vec<String> {
        ops.iter()
            .map(|op| match op {
                Operation::CreateModel { table_name, fields } => format!(
                    "create {table_name}({})",
                    fields
                        .iter()
                        .map(|f| f.name.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                Operation::AddField { table_name, field } => {
                    format!("add {table_name}.{}", field.name)
                }
                Operation::RemoveField { table_name, field } => {
                    format!("remove {table_name}.{}", field.name)
                }
                Operation::RemoveModel { table_name, .. } => format!("drop {table_name}"),
            })
            .collect()
    }

    #[test]
    fn no_changes() {
        let a = table("app__a", vec![field("id")]);
        assert!(generate_operations(&[&a], &[&a]).unwrap().is_empty());
    }

    #[test]
    fn creates_ordered_by_foreign_keys() {
        let a = table("app__a", vec![field("id"), fk("b", "app__b")]);
        let b = table("app__b", vec![field("id")]);
        let ops = generate_operations(&[&a, &b], &[]).unwrap();
        assert_eq!(summary(&ops), ["create app__b(id)", "create app__a(id,b)"]);
    }

    #[test]
    fn foreign_key_cycle_is_deferred() {
        let a = table("app__a", vec![field("id"), fk("b", "app__b")]);
        let b = table("app__b", vec![field("id"), fk("a", "app__a")]);
        let ops = summary(&generate_operations(&[&a, &b], &[]).unwrap());
        assert_eq!(ops.len(), 3);
        assert!(ops[2].starts_with("add "), "{ops:?}");
    }

    #[test]
    fn add_remove_fields_and_models() {
        let prev_a = table("app__a", vec![field("id"), field("old")]);
        let prev_b = table("app__b", vec![field("id")]);
        let a = table("app__a", vec![field("id"), field("new")]);
        let ops = generate_operations(&[&a], &[&prev_a, &prev_b]).unwrap();
        assert_eq!(
            summary(&ops),
            ["add app__a.new", "remove app__a.old", "drop app__b"]
        );
    }

    #[test]
    fn changed_field_is_rejected() {
        let prev = table("app__a", vec![field("id")]);
        let a = table(
            "app__a",
            vec![FieldState {
                null: true,
                ..field("id")
            }],
        );
        assert!(generate_operations(&[&a], &[&prev]).is_err());
    }

    #[test]
    fn migration_files_sorted_and_filtered() {
        let dir = tempfile::tempdir().unwrap();
        for file in ["m_0002_b.rs", "m_0001_a.rs", "other.rs", "m_0003_c.txt"] {
            std::fs::write(dir.path().join(file), "").unwrap();
        }
        assert_eq!(
            migration_files(dir.path()).unwrap(),
            ["m_0001_a", "m_0002_b"]
        );
        assert_eq!(
            migration_files(&dir.path().join("missing")).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn frozen_field_types() {
        let generator = MigrationGenerator {
            app_name: "app".to_owned(),
            src_path: PathBuf::new(),
            schema: ProjectSchema {
                version: 1,
                models: vec![table("app__parent", vec![field("id")])],
                migrated_models: vec![TableState {
                    app: "cot".to_owned(),
                    ..table("cot__database_user", vec![field("id")])
                }],
                migrations: vec![],
            },
        };
        let norm = |t: &TokenStream| t.to_string().replace(' ', "");
        let tokens = |f: FieldState| norm(&generator.frozen_field(&f).unwrap());

        assert_eq!(
            tokens(FieldState {
                auto_value: true,
                primary_key: true,
                ..field("id")
            }),
            norm(&quote!(#[model(primary_key)] id: db::Auto<i32>))
        );
        assert_eq!(
            tokens(FieldState {
                ty: ColumnType::DateTimeWithTimeZone,
                null: true,
                ..field("type")
            }),
            norm(&quote!(
                r#type: Option<chrono::DateTime<chrono::FixedOffset>>
            ))
        );
        assert_eq!(
            tokens(fk("parent", "app__parent")),
            norm(&quote!(parent: db::ForeignKey<_Parent>))
        );
        assert_eq!(
            tokens(fk("user", "cot__database_user")),
            norm(&quote!(user: db::ForeignKey<_CotDatabaseUser>))
        );
    }
}
