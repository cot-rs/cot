use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Debug, Display};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use cot::db::migrations::{DynMigration, MigrationEngine};
use cot_codegen::model::{Field, Model, ModelArgs, ModelOpts, ModelType};
use cot_codegen::symbol_resolver::SymbolResolver;
use darling::FromMeta;
use petgraph::graph::DiGraph;
use petgraph::visit::EdgeRef;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{parse_quote, Meta};
use tracing::{debug, trace};

use crate::utils::{print_status_msg, StatusType, WorkspaceManager};

pub fn make_migrations(path: &Path, options: MigrationGeneratorOptions) -> anyhow::Result<()> {
    if let Some(manager) = WorkspaceManager::from_path(path)? {
        let manifest = match &options.app_name {
            Some(app_name) => manager.get_package_manifest(app_name),
            None => manager.get_package_manifest_by_path(path),
        }
        .context("unable to find package manifest")?;

        let crate_name = manifest
            .package
            .as_ref()
            .context("unable to find package in Cargo.toml")?
            .name
            .clone();
        let manifest_path = manager
            .get_manifest_path(&crate_name)
            .expect("manifest must exist by this point");

        let generator = MigrationGenerator::new(PathBuf::from(manifest_path), crate_name, options);
        let migrations = generator
            .generate_migrations_as_source()
            .context("unable to generate migrations")?;
        generator
            .write_migrations(&migrations)
            .context("unable to write migrations")?;
        generator
            .write_migrations_module()
            .context("unable to write migrations.rs")?;
    }

    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct MigrationGeneratorOptions {
    pub app_name: Option<String>,
    pub output_dir: Option<PathBuf>,
}

#[derive(Debug)]
pub struct MigrationGenerator {
    cargo_toml_path: PathBuf,
    crate_name: String,
    options: MigrationGeneratorOptions,
}

const MIGRATIONS_MODULE_NAME: &str = "migrations";
const MIGRATIONS_MODULE_PREFIX: &str = "m_";

impl MigrationGenerator {
    #[must_use]
    pub fn new(
        cargo_toml_path: PathBuf,
        crate_name: String,
        options: MigrationGeneratorOptions,
    ) -> Self {
        Self {
            cargo_toml_path,
            crate_name,
            options,
        }
    }

    pub fn generate_migrations_as_source(&self) -> anyhow::Result<MigrationAsSource> {
        let source_files = self.get_source_files()?;
        self.generate_migrations_as_source_from_files(source_files)
    }

    pub fn generate_migrations_as_source_from_files(
        &self,
        source_files: Vec<SourceFile>,
    ) -> anyhow::Result<MigrationAsSource> {
        if let Some(migration) = self.generate_migrations_as_generated_from_files(source_files)? {
            let migration_name = migration.migration_name.clone();
            let content = self.generate_migration_file_content(migration);
            Ok(MigrationAsSource::new(migration_name, content))
        } else {
            bail!("unable to generate migrations from source files")
        }
    }

    /// Generate migrations and return internal structures that can be used to
    /// generate source code.
    pub fn generate_migrations_as_generated_from_files(
        &self,
        source_files: Vec<SourceFile>,
    ) -> anyhow::Result<Option<GeneratedMigration>> {
        let AppState { models, migrations } = self.process_source_files(source_files)?;
        let migration_processor = MigrationProcessor::new(migrations)?;
        let migration_models = migration_processor.latest_models();

        let (modified_models, operations) = Self::generate_operations(&models, &migration_models);
        if operations.is_empty() {
            Ok(None)
        } else {
            let migration_name = migration_processor.next_migration_name()?;
            let dependencies = migration_processor.base_dependencies();

            let migration =
                GeneratedMigration::new(migration_name, modified_models, dependencies, operations);
            Ok(Some(migration))
        }
    }

    pub fn write_migrations(&self, migration: &MigrationAsSource) -> anyhow::Result<()> {
        print_status_msg(
            StatusType::Creating,
            &format!("Migration '{}'", migration.name),
        );

        self.save_migration_to_file(&migration.name, migration.content.as_ref())?;

        print_status_msg(
            StatusType::Created,
            &format!("Migration '{}'", migration.name),
        );

        Ok(())
    }

    pub fn write_migrations_module(&self) -> anyhow::Result<()> {
        let src_path = self.get_src_path();
        let migrations_dir = src_path.join(MIGRATIONS_MODULE_NAME);

        let migration_list = Self::get_migration_list(&migrations_dir)?;
        let contents = Self::get_migration_module_contents(&migration_list);
        let contents_string = Self::format_tokens(contents);

        let header = Self::migration_header();
        let migration_header = "//! List of migrations for the current app.\n//!";
        let contents_with_header = format!("{migration_header}\n{header}\n\n{contents_string}");

        let mut file = File::create(src_path.join(format!("{MIGRATIONS_MODULE_NAME}.rs")))?;
        file.write_all(contents_with_header.as_bytes())?;

        Ok(())
    }

    fn get_source_files(&self) -> anyhow::Result<Vec<SourceFile>> {
        let src_dir = self
            .cargo_toml_path
            .parent()
            .with_context(|| "unable to find parent dir")?
            .join("src");
        let src_dir = src_dir
            .canonicalize()
            .with_context(|| "unable to canonicalize src dir")?;

        let source_file_paths = Self::find_source_files(&src_dir)?;
        let source_files = source_file_paths
            .into_iter()
            .map(|path| {
                Self::parse_file(&src_dir, path.clone())
                    .with_context(|| format!("unable to parse file: {}", path.display()))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(source_files)
    }

    pub fn find_source_files(src_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for entry in glob::glob(src_dir.join("**/*.rs").to_str().unwrap())
            .with_context(|| "unable to find Rust source files with glob")?
        {
            let path = entry?;
            paths.push(
                path.strip_prefix(src_dir)
                    .expect("path must be in src dir")
                    .to_path_buf(),
            );
        }

        Ok(paths)
    }

    fn process_source_files(&self, source_files: Vec<SourceFile>) -> anyhow::Result<AppState> {
        let mut app_state = AppState::new();

        for source_file in source_files {
            let path = source_file.path.clone();
            self.process_parsed_file(source_file, &mut app_state)
                .with_context(|| format!("unable to find models in file: {}", path.display()))?;
        }

        Ok(app_state)
    }

    fn parse_file(src_dir: &Path, path: PathBuf) -> anyhow::Result<SourceFile> {
        let full_path = src_dir.join(&path);
        debug!("Parsing file: {:?}", &full_path);
        let mut file = File::open(&full_path).with_context(|| "unable to open file")?;

        let mut src = String::new();
        file.read_to_string(&mut src)
            .with_context(|| format!("unable to read file: {}", full_path.display()))?;

        SourceFile::parse(path, &src)
    }

    fn process_parsed_file(
        &self,
        SourceFile {
            path,
            content: file,
        }: SourceFile,
        app_state: &mut AppState,
    ) -> anyhow::Result<()> {
        trace!("Processing file: {:?}", &path);

        let symbol_resolver = SymbolResolver::from_file(&file, &path);

        let mut migration_models = Vec::new();
        for item in file.items {
            if let syn::Item::Struct(mut item) = item {
                for attr in &item.attrs.clone() {
                    if is_model_attr(attr) {
                        symbol_resolver.resolve_struct(&mut item);

                        let args = Self::model_args_from_attr(&path, attr)?;
                        let model_in_source =
                            ModelInSource::from_item(item, &args, &symbol_resolver)?;

                        match args.model_type {
                            ModelType::Application => {
                                trace!(
                                    "Found an Application model: {}",
                                    model_in_source.model.name.to_string()
                                );
                                app_state.models.push(model_in_source);
                            }
                            ModelType::Migration => {
                                trace!(
                                    "Found a Migration model: {}",
                                    model_in_source.model.name.to_string()
                                );
                                migration_models.push(model_in_source);
                            }
                            ModelType::Internal => {}
                        }

                        break;
                    }
                }
            }
        }

        if !migration_models.is_empty() {
            let migration_name = path
                .file_stem()
                .with_context(|| format!("unable to get migration file name: {}", path.display()))?
                .to_string_lossy()
                .to_string();
            app_state.migrations.push(Migration {
                app_name: self.crate_name.clone(),
                name: migration_name,
                models: migration_models,
            });
        }

        Ok(())
    }

    fn model_args_from_attr(path: &Path, attr: &syn::Attribute) -> Result<ModelArgs, ParsingError> {
        match attr.meta {
            Meta::Path(_) => {
                // Means `#[model]` without any arguments
                Ok(ModelArgs::default())
            }
            _ => ModelArgs::from_meta(&attr.meta).map_err(|e| {
                ParsingError::from_darling(
                    "couldn't parse model macro arguments",
                    path.to_owned(),
                    &e,
                )
            }),
        }
    }

    #[must_use]
    fn generate_operations(
        app_models: &Vec<ModelInSource>,
        migration_models: &Vec<ModelInSource>,
    ) -> (Vec<ModelInSource>, Vec<DynOperation>) {
        let mut operations = Vec::new();
        let mut modified_models = Vec::new();

        let mut all_model_names = HashSet::new();
        let mut app_models_map = HashMap::new();
        for model in app_models {
            all_model_names.insert(model.model.table_name.clone());
            app_models_map.insert(model.model.table_name.clone(), model);
        }
        let mut migration_models_map = HashMap::new();
        for model in migration_models {
            all_model_names.insert(model.model.table_name.clone());
            migration_models_map.insert(model.model.table_name.clone(), model);
        }
        let mut all_model_names: Vec<_> = all_model_names.into_iter().collect();
        all_model_names.sort();

        for model_name in all_model_names {
            let app_model = app_models_map.get(&model_name);
            let migration_model = migration_models_map.get(&model_name);

            match (app_model, migration_model) {
                (Some(&app_model), None) => {
                    operations.push(MigrationOperationGenerator::make_create_model_operation(
                        app_model,
                    ));
                    modified_models.push(app_model.clone());
                }
                (Some(&app_model), Some(&migration_model)) => {
                    if app_model.model != migration_model.model {
                        modified_models.push(app_model.clone());
                        operations.extend(
                            MigrationOperationGenerator::make_alter_model_operations(
                                app_model,
                                migration_model,
                            ),
                        );
                    }
                }
                (None, Some(&migration_model)) => {
                    operations.push(MigrationOperationGenerator::make_remove_model_operation(
                        migration_model,
                    ));
                }
                (None, None) => unreachable!(),
            }
        }

        (modified_models, operations)
    }

    fn generate_migration_file_content(&self, migration: GeneratedMigration) -> String {
        let operations: Vec<_> = migration
            .operations
            .into_iter()
            .map(|operation| operation.repr())
            .collect();
        let dependencies: Vec<_> = migration
            .dependencies
            .into_iter()
            .map(|dependency| dependency.repr())
            .collect();

        let app_name = self.options.app_name.as_ref().unwrap_or(&self.crate_name);
        let migration_name = &migration.migration_name;
        let migration_def = quote! {
            #[derive(Debug, Copy, Clone)]
            pub(super) struct Migration;

            impl ::cot::db::migrations::Migration for Migration {
                const APP_NAME: &'static str = #app_name;
                const MIGRATION_NAME: &'static str = #migration_name;
                const DEPENDENCIES: &'static [::cot::db::migrations::MigrationDependency] = &[
                    #(#dependencies,)*
                ];
                const OPERATIONS: &'static [::cot::db::migrations::Operation] = &[
                    #(#operations,)*
                ];
            }
        };

        let models = migration
            .modified_models
            .iter()
            .map(Self::model_to_migration_model)
            .collect::<Vec<_>>();
        let models_def = quote! {
            #(#models)*
        };

        Self::generate_migration(migration_def, models_def)
    }

    fn save_migration_to_file(&self, migration_name: &String, bytes: &[u8]) -> anyhow::Result<()> {
        let src_path = self.get_src_path();
        let migration_path = src_path.join(MIGRATIONS_MODULE_NAME);
        let migration_file = migration_path.join(format!("{migration_name}.rs"));
        print_status_msg(
            StatusType::Creating,
            &format!("Migration file '{}'", migration_file.display()),
        );
        std::fs::create_dir_all(&migration_path).with_context(|| {
            format!(
                "unable to create migrations directory: {}",
                migration_path.display()
            )
        })?;

        let mut file = File::create(&migration_file).with_context(|| {
            format!(
                "unable to create migration file: {}",
                migration_file.display()
            )
        })?;
        file.write_all(bytes)
            .with_context(|| "unable to write migration file")?;
        print_status_msg(
            StatusType::Created,
            &format!("Migration file '{}'", migration_file.display()),
        );
        Ok(())
    }

    #[must_use]
    fn generate_migration(migration: TokenStream, modified_models: TokenStream) -> String {
        let migration = Self::format_tokens(migration);
        let modified_models = Self::format_tokens(modified_models);

        let header = Self::migration_header();

        format!("{header}\n\n{migration}\n{modified_models}")
    }

    fn migration_header() -> String {
        let version = env!("CARGO_PKG_VERSION");
        let date_time = chrono::offset::Utc::now().format("%Y-%m-%d %H:%M:%S%:z");
        let header = format!("//! Generated by cot CLI {version} on {date_time}");
        header
    }

    #[must_use]
    fn format_tokens(tokens: TokenStream) -> String {
        let parsed: syn::File = syn::parse2(tokens).unwrap();
        prettyplease::unparse(&parsed)
    }

    #[must_use]
    fn model_to_migration_model(model: &ModelInSource) -> TokenStream {
        let mut model_source = model.model_item.clone();
        model_source.vis = syn::Visibility::Inherited;
        model_source.ident = format_ident!("_{}", model_source.ident);
        model_source.attrs.clear();
        model_source
            .attrs
            .push(syn::parse_quote! {#[derive(::core::fmt::Debug)]});
        model_source
            .attrs
            .push(syn::parse_quote! {#[::cot::db::model(model_type = "migration")]});
        quote! {
            #model_source
        }
    }

    fn get_migration_list(migrations_dir: &PathBuf) -> anyhow::Result<Vec<String>> {
        Ok(std::fs::read_dir(migrations_dir)
            .with_context(|| {
                format!(
                    "unable to read migrations directory: {}",
                    migrations_dir.display()
                )
            })?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                let stem = path.file_stem();

                if path.is_file()
                    && stem
                        .unwrap_or_default()
                        .to_string_lossy()
                        .starts_with(MIGRATIONS_MODULE_PREFIX)
                    && path.extension() == Some("rs".as_ref())
                {
                    stem.map(|stem| stem.to_string_lossy().to_string())
                } else {
                    None
                }
            })
            .collect())
    }

    #[must_use]
    fn get_migration_module_contents(migration_list: &[String]) -> TokenStream {
        let migration_mods = migration_list.iter().map(|migration| {
            let migration = format_ident!("{}", migration);
            quote! {
                pub mod #migration;
            }
        });
        let migration_refs = migration_list.iter().map(|migration| {
            let migration = format_ident!("{}", migration);
            quote! {
                &#migration::Migration
            }
        });

        quote! {
            #(#migration_mods)*

            /// The list of migrations for current app.
            pub const MIGRATIONS: &[&::cot::db::migrations::SyncDynMigration] = &[
                #(#migration_refs),*
            ];
        }
    }

    fn get_src_path(&self) -> PathBuf {
        self.options.output_dir.clone().unwrap_or(
            self.cargo_toml_path
                .parent()
                .expect("Cargo.toml should always have parent project directory")
                .join("src"),
        )
    }
}

struct MigrationOperationGenerator;

impl MigrationOperationGenerator {
    #[must_use]
    fn make_create_model_operation(app_model: &ModelInSource) -> DynOperation {
        print_status_msg(
            StatusType::Creating,
            &format!("Model '{}'", app_model.model.table_name),
        );
        let op = DynOperation::CreateModel {
            table_name: app_model.model.table_name.clone(),
            model_ty: app_model.model.resolved_ty.clone(),
            fields: app_model.model.fields.clone(),
        };
        print_status_msg(
            StatusType::Created,
            &format!("Model '{}'", app_model.model.table_name),
        );
        op
    }

    #[must_use]
    fn make_alter_model_operations(
        app_model: &ModelInSource,
        migration_model: &ModelInSource,
    ) -> Vec<DynOperation> {
        let mut all_field_names = HashSet::new();
        let mut app_model_fields = HashMap::new();
        print_status_msg(
            StatusType::Modifying,
            &format!("Model '{}'", app_model.model.table_name),
        );

        for field in &app_model.model.fields {
            all_field_names.insert(field.column_name.clone());
            app_model_fields.insert(field.column_name.clone(), field);
        }
        let mut migration_model_fields = HashMap::new();
        for field in &migration_model.model.fields {
            all_field_names.insert(field.column_name.clone());
            migration_model_fields.insert(field.column_name.clone(), field);
        }

        let mut all_field_names: Vec<_> = all_field_names.into_iter().collect();
        // sort to ensure deterministic order
        all_field_names.sort();

        let mut operations = Vec::new();
        for field_name in all_field_names {
            let app_field = app_model_fields.get(&field_name);
            let migration_field = migration_model_fields.get(&field_name);

            match (app_field, migration_field) {
                (Some(app_field), None) => {
                    operations.push(Self::make_add_field_operation(app_model, app_field));
                }
                (Some(app_field), Some(migration_field)) => {
                    let operation = Self::make_alter_field_operation(
                        app_model,
                        app_field,
                        migration_model,
                        migration_field,
                    );
                    if let Some(operation) = operation {
                        operations.push(operation);
                    }
                }
                (None, Some(migration_field)) => {
                    operations.push(Self::make_remove_field_operation(
                        migration_model,
                        migration_field,
                    ));
                }
                (None, None) => unreachable!(),
            }
        }
        print_status_msg(
            StatusType::Modified,
            &format!("Model '{}'", app_model.model.table_name),
        );

        operations
    }

    #[must_use]
    fn make_remove_model_operation(migration_model: &ModelInSource) -> DynOperation {
        print_status_msg(
            StatusType::Removing,
            &format!("Model '{}'", &migration_model.model.name),
        );

        let op = DynOperation::RemoveModel {
            table_name: migration_model.model.table_name.clone(),
            model_ty: migration_model.model.resolved_ty.clone(),
            fields: migration_model.model.fields.clone(),
        };

        print_status_msg(
            StatusType::Removed,
            &format!("Model '{}'", &migration_model.model.name),
        );

        op
    }

    #[must_use]
    fn make_add_field_operation(app_model: &ModelInSource, field: &Field) -> DynOperation {
        print_status_msg(
            StatusType::Adding,
            &format!(
                "Field '{}' to Model '{}'",
                &field.field_name, app_model.model.name
            ),
        );

        let op = DynOperation::AddField {
            table_name: app_model.model.table_name.clone(),
            model_ty: app_model.model.resolved_ty.clone(),
            field: Box::new(field.clone()),
        };

        print_status_msg(
            StatusType::Added,
            &format!(
                "Field '{}' to Model '{}'",
                &field.field_name, app_model.model.name
            ),
        );

        op
    }

    #[must_use]
    fn make_alter_field_operation(
        _app_model: &ModelInSource,
        app_field: &Field,
        migration_model: &ModelInSource,
        migration_field: &Field,
    ) -> Option<DynOperation> {
        if app_field == migration_field {
            return None;
        }
        print_status_msg(
            StatusType::Modifying,
            &format!(
                "Field '{}' from Model '{}'",
                &migration_field.field_name, migration_model.model.name
            ),
        );

        todo!();

        // line below should be removed once todo is implemented
        #[allow(unreachable_code)]
        print_status_msg(
            StatusType::Modified,
            &format!(
                "Field '{}' from Model '{}'",
                &migration_field.field_name, migration_model.model.name
            ),
        );
    }

    #[must_use]
    fn make_remove_field_operation(
        migration_model: &ModelInSource,
        migration_field: &Field,
    ) -> DynOperation {
        print_status_msg(
            StatusType::Removing,
            &format!(
                "Field '{}' from Model '{}'",
                &migration_field.field_name, migration_model.model.name
            ),
        );

        let op = DynOperation::RemoveField {
            table_name: migration_model.model.table_name.clone(),
            model_ty: migration_model.model.resolved_ty.clone(),
            field: Box::new(migration_field.clone()),
        };

        print_status_msg(
            StatusType::Removed,
            &format!(
                "Field '{}' from Model '{}'",
                &migration_field.field_name, migration_model.model.name
            ),
        );

        op
    }
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    path: PathBuf,
    content: syn::File,
}

impl SourceFile {
    #[must_use]
    fn new(path: PathBuf, content: syn::File) -> Self {
        assert!(
            path.is_relative(),
            "path must be relative to the src directory"
        );
        Self { path, content }
    }

    pub fn parse(path: PathBuf, content: &str) -> anyhow::Result<Self> {
        Ok(Self::new(
            path,
            syn::parse_file(content).with_context(|| "unable to parse file")?,
        ))
    }
}

#[derive(Debug, Clone)]
struct AppState {
    /// All the application models found in the source
    models: Vec<ModelInSource>,
    /// All the migrations found in the source
    migrations: Vec<Migration>,
}

impl AppState {
    #[must_use]
    fn new() -> Self {
        Self {
            models: Vec::new(),
            migrations: Vec::new(),
        }
    }
}

/// Helper struct to process already existing migrations.
#[derive(Debug, Clone)]
struct MigrationProcessor {
    migrations: Vec<Migration>,
}

impl MigrationProcessor {
    fn new(mut migrations: Vec<Migration>) -> anyhow::Result<Self> {
        MigrationEngine::sort_migrations(&mut migrations)?;
        Ok(Self { migrations })
    }

    /// Returns the latest (in the order of applying migrations) versions of the
    /// models that are marked as migration models, that means the latest
    /// version of each migration model.
    ///
    /// This is useful for generating migrations - we can compare the latest
    /// version of the model in the source code with the latest version of the
    /// model in the migrations (returned by this method) and generate the
    /// necessary operations.
    #[must_use]
    fn latest_models(&self) -> Vec<ModelInSource> {
        let mut migration_models: HashMap<String, &ModelInSource> = HashMap::new();
        for migration in &self.migrations {
            for model in &migration.models {
                migration_models.insert(model.model.table_name.clone(), model);
            }
        }

        migration_models.into_values().cloned().collect()
    }

    fn next_migration_name(&self) -> anyhow::Result<String> {
        if self.migrations.is_empty() {
            return Ok(format!("{MIGRATIONS_MODULE_PREFIX}0001_initial"));
        }

        let last_migration = self.migrations.last().unwrap();
        let last_migration_number = last_migration
            .name
            .split('_')
            .nth(1)
            .with_context(|| format!("migration number not found: {}", last_migration.name))?
            .parse::<u32>()
            .with_context(|| {
                format!("unable to parse migration number: {}", last_migration.name)
            })?;

        let migration_number = last_migration_number + 1;
        let now = chrono::Utc::now();
        let date_time = now.format("%Y%m%d_%H%M%S");

        Ok(format!(
            "{MIGRATIONS_MODULE_PREFIX}{migration_number:04}_auto_{date_time}"
        ))
    }

    /// Returns the list of dependencies for the next migration, based on the
    /// already existing and processed migrations.
    fn base_dependencies(&self) -> Vec<DynDependency> {
        if self.migrations.is_empty() {
            return Vec::new();
        }

        let last_migration = self.migrations.last().unwrap();
        vec![DynDependency::Migration {
            app: last_migration.app_name.clone(),
            migration: last_migration.name.clone(),
        }]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelInSource {
    model_item: syn::ItemStruct,
    model: Model,
}

impl ModelInSource {
    fn from_item(
        item: syn::ItemStruct,
        args: &ModelArgs,
        symbol_resolver: &SymbolResolver,
    ) -> anyhow::Result<Self> {
        let input: syn::DeriveInput = item.clone().into();
        let opts = ModelOpts::new_from_derive_input(&input)
            .map_err(|e| anyhow::anyhow!("cannot parse model: {}", e))?;
        let model = opts.as_model(args, symbol_resolver)?;

        Ok(Self {
            model_item: item,
            model,
        })
    }
}

/// A migration generated by the CLI and before converting to a Rust
/// source code and writing to a file.
#[derive(Debug, Clone)]
pub struct GeneratedMigration {
    pub migration_name: String,
    pub modified_models: Vec<ModelInSource>,
    pub dependencies: Vec<DynDependency>,
    pub operations: Vec<DynOperation>,
}

impl GeneratedMigration {
    #[must_use]
    fn new(
        migration_name: String,
        modified_models: Vec<ModelInSource>,
        mut dependencies: Vec<DynDependency>,
        mut operations: Vec<DynOperation>,
    ) -> Self {
        Self::remove_cycles(&mut operations);
        Self::toposort_operations(&mut operations);
        dependencies.extend(Self::get_foreign_key_dependencies(&operations));

        Self {
            migration_name,
            modified_models,
            dependencies,
            operations,
        }
    }

    /// Get the list of [`DynDependency`] for all foreign keys that point
    /// to models that are **not** created in this migration.
    fn get_foreign_key_dependencies(operations: &[DynOperation]) -> Vec<DynDependency> {
        let create_ops = Self::get_create_ops_map(operations);
        let ops_adding_foreign_keys = Self::get_ops_adding_foreign_keys(operations);

        let mut dependencies = Vec::new();
        for (_index, dependency_ty) in &ops_adding_foreign_keys {
            if !create_ops.contains_key(dependency_ty) {
                dependencies.push(DynDependency::Model {
                    model_type: dependency_ty.clone(),
                });
            }
        }

        dependencies
    }

    /// Removes dependency cycles by removing operations that create cycles.
    ///
    /// This method tries to minimize the number of operations added by
    /// calculating the minimum feedback arc set of the dependency graph.
    ///
    /// This method modifies the `operations` parameter in place.
    ///
    /// # See also
    ///
    /// * [`Self::remove_dependency`]
    fn remove_cycles(operations: &mut Vec<DynOperation>) {
        let graph = Self::construct_dependency_graph(operations);

        let cycle_edges = petgraph::algo::feedback_arc_set::greedy_feedback_arc_set(&graph);
        for edge_id in cycle_edges {
            let (from, to) = graph
                .edge_endpoints(edge_id.id())
                .expect("greedy_feedback_arc_set should always return valid edge refs");

            let to_op = operations[to.index()].clone();
            let from_op = &mut operations[from.index()];
            debug!(
                "Removing cycle by removing operation {:?} that depends on {:?}",
                from_op, to_op
            );

            let to_add = Self::remove_dependency(from_op, &to_op);
            operations.extend(to_add);
        }
    }

    /// Remove a dependency between two operations.
    ///
    /// This is done by removing foreign keys from the `from` operation that
    /// point to the model created by `to` operation, and creating a new
    /// `AddField` operation for each removed foreign key.
    #[must_use]
    fn remove_dependency(from: &mut DynOperation, to: &DynOperation) -> Vec<DynOperation> {
        match from {
            DynOperation::CreateModel {
                table_name,
                model_ty,
                fields,
            } => {
                let to_type = match to {
                    DynOperation::CreateModel { model_ty, .. } => model_ty,
                    DynOperation::AddField { .. } => {
                        unreachable!(
                            "AddField operation shouldn't be a dependency of CreateModel \
                            because it doesn't create a new model"
                        )
                    }
                    DynOperation::RemoveField { .. } => {
                        unreachable!(
                            "RemoveField operation shouldn't be a dependency of CreateModel \
                        because it doesn't create a new model"
                        )
                    }
                    DynOperation::RemoveModel { .. } => {
                        unreachable!(
                            "RemoveModel operation shouldn't be a dependency of CreateModel \
                        because it doesn't create a new model"
                        )
                    }
                };
                trace!(
                    "Removing foreign keys from {} to {}",
                    model_ty.to_token_stream().to_string(),
                    to_type.into_token_stream().to_string()
                );

                let mut result = Vec::new();
                let (fields_to_remove, fields_to_retain): (Vec<_>, Vec<_>) = std::mem::take(fields)
                    .into_iter()
                    .partition(|field| is_field_foreign_key_to(field, to_type));
                *fields = fields_to_retain;

                for field in fields_to_remove {
                    result.push(DynOperation::AddField {
                        table_name: table_name.clone(),
                        model_ty: model_ty.clone(),
                        field: Box::new(field),
                    });
                }

                result
            }
            DynOperation::AddField { .. } => {
                // AddField only links two already existing models together, so
                // removing it shouldn't ever affect whether a graph is cyclic
                unreachable!("AddField operation should never create cycles")
            }
            DynOperation::RemoveField { .. } => {
                // RemoveField doesn't create dependencies, it only removes a field
                unreachable!("RemoveField operation should never create cycles")
            }
            DynOperation::RemoveModel { .. } => {
                // RemoveModel doesn't create dependencies, it only removes a model
                unreachable!("RemoveModel operation should never create cycles")
            }
        }
    }

    /// Topologically sort operations in this migration.
    ///
    /// This is to ensure that operations will be applied in the correct order.
    /// If there are no dependencies between operations, the order of operations
    /// will not be modified.
    ///
    /// This method modifies the `operations` field in place.
    ///
    /// # Panics
    ///
    /// This method should be called after removing cycles; otherwise it will
    /// panic.
    fn toposort_operations(operations: &mut [DynOperation]) {
        let graph = Self::construct_dependency_graph(operations);

        let sorted = petgraph::algo::toposort(&graph, None)
            .expect("cycles shouldn't exist after removing them");
        let mut sorted = sorted
            .into_iter()
            .map(petgraph::graph::NodeIndex::index)
            .collect::<Vec<_>>();
        cot::__private::apply_permutation(operations, &mut sorted);
    }

    /// Construct a graph that represents reverse dependencies between
    /// given operations.
    ///
    /// The graph is directed and has an edge from operation A to operation B
    /// if operation B creates a foreign key that points to a model created by
    /// operation A.
    #[must_use]
    fn construct_dependency_graph(operations: &[DynOperation]) -> DiGraph<usize, (), usize> {
        let create_ops = Self::get_create_ops_map(operations);
        let ops_adding_foreign_keys = Self::get_ops_adding_foreign_keys(operations);

        let mut graph = DiGraph::with_capacity(operations.len(), 0);

        for i in 0..operations.len() {
            graph.add_node(i);
        }
        for (i, dependency_ty) in &ops_adding_foreign_keys {
            if let Some(&dependency) = create_ops.get(dependency_ty) {
                graph.add_edge(
                    petgraph::graph::NodeIndex::new(dependency),
                    petgraph::graph::NodeIndex::new(*i),
                    (),
                );
            }
        }

        graph
    }

    /// Return a map of (resolved) model types to the index of the
    /// operation that creates given model.
    #[must_use]
    fn get_create_ops_map(operations: &[DynOperation]) -> HashMap<syn::Type, usize> {
        #[allow(clippy::match_wildcard_for_single_variants)] // we only care about CreateModel here
        operations
            .iter()
            .enumerate()
            .filter_map(|(i, op)| match op {
                DynOperation::CreateModel { model_ty, .. } => Some((model_ty.clone(), i)),
                _ => None,
            })
            .collect()
    }

    /// Return a list of operations that add foreign keys as tuples of
    /// operation index and the type of the model that foreign key points to.
    #[must_use]
    fn get_ops_adding_foreign_keys(operations: &[DynOperation]) -> Vec<(usize, syn::Type)> {
        operations
            .iter()
            .enumerate()
            .flat_map(|(i, op)| match op {
                DynOperation::CreateModel { fields, .. } => fields
                    .iter()
                    .filter_map(foreign_key_for_field)
                    .map(|to_model| (i, to_model))
                    .collect::<Vec<(usize, syn::Type)>>(),
                DynOperation::AddField {
                    field, model_ty, ..
                } => {
                    let mut ops = vec![(i, model_ty.clone())];

                    if let Some(to_type) = foreign_key_for_field(field) {
                        ops.push((i, to_type));
                    }

                    ops
                }
                DynOperation::RemoveField { .. } => {
                    // RemoveField Doesnt Add Foreign Keys
                    Vec::new()
                }
                DynOperation::RemoveModel { .. } => {
                    // RemoveModel Doesnt Add Foreign Keys
                    Vec::new()
                }
            })
            .collect()
    }
}

/// A migration represented as a generated and ready to write Rust source code.
#[derive(Debug, Clone)]
pub struct MigrationAsSource {
    pub name: String,
    pub content: String,
}

impl MigrationAsSource {
    #[must_use]
    pub(crate) fn new(name: String, content: String) -> Self {
        Self { name, content }
    }
}

#[must_use]
fn is_model_attr(attr: &syn::Attribute) -> bool {
    let path = attr.path();

    let model_path: syn::Path = parse_quote!(cot::db::model);
    let model_path_prefixed: syn::Path = parse_quote!(::cot::db::model);

    attr.style == syn::AttrStyle::Outer
        && (path.is_ident("model") || path == &model_path || path == &model_path_prefixed)
}

trait Repr {
    fn repr(&self) -> TokenStream;
}

impl Repr for Field {
    fn repr(&self) -> TokenStream {
        let column_name = &self.column_name;
        let ty = &self.ty;
        let mut tokens = quote! {
            ::cot::db::migrations::Field::new(::cot::db::Identifier::new(#column_name), <#ty as ::cot::db::DatabaseField>::TYPE)
        };
        if self.auto_value {
            tokens = quote! { #tokens.auto() }
        }
        if self.primary_key {
            tokens = quote! { #tokens.primary_key() }
        }
        if let Some(fk_spec) = self.foreign_key.clone() {
            let to_model = &fk_spec.to_model;

            tokens = quote! {
                #tokens.foreign_key(
                    <#to_model as ::cot::db::Model>::TABLE_NAME,
                    <#to_model as ::cot::db::Model>::PRIMARY_KEY_NAME,
                    ::cot::db::ForeignKeyOnDeletePolicy::Restrict,
                    ::cot::db::ForeignKeyOnUpdatePolicy::Restrict,
                )
            }
        }
        tokens = quote! { #tokens.set_null(<#ty as ::cot::db::DatabaseField>::NULLABLE) };
        if self.unique {
            tokens = quote! { #tokens.unique() }
        }
        tokens
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Migration {
    app_name: String,
    name: String,
    models: Vec<ModelInSource>,
}

impl DynMigration for Migration {
    fn app_name(&self) -> &str {
        &self.app_name
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn dependencies(&self) -> &[cot::db::migrations::MigrationDependency] {
        &[]
    }

    fn operations(&self) -> &[cot::db::migrations::Operation] {
        &[]
    }
}

/// A version of [`cot::db::migrations::MigrationDependency`] that can be
/// created at runtime and is using codegen types.
///
/// This is used to generate migration files.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
// this is not frequently used, so we don't mind extra memory usage
#[allow(clippy::large_enum_variant)]
pub enum DynDependency {
    Migration { app: String, migration: String },
    Model { model_type: syn::Type },
}

impl Repr for DynDependency {
    fn repr(&self) -> TokenStream {
        match self {
            Self::Migration { app, migration } => {
                quote! {
                    ::cot::db::migrations::MigrationDependency::migration(#app, #migration)
                }
            }
            Self::Model { model_type } => {
                quote! {
                    ::cot::db::migrations::MigrationDependency::model(
                        <#model_type as ::cot::db::Model>::APP_NAME,
                        <#model_type as ::cot::db::Model>::TABLE_NAME
                    )
                }
            }
        }
    }
}

/// A version of [`cot::db::migrations::Operation`] that can be created at
/// runtime and is using codegen types.
///
/// This is used to generate migration files.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DynOperation {
    CreateModel {
        table_name: String,
        model_ty: syn::Type,
        fields: Vec<Field>,
    },
    AddField {
        table_name: String,
        model_ty: syn::Type,
        // boxed to reduce the size difference between enum variants
        field: Box<Field>,
    },
    RemoveField {
        table_name: String,
        model_ty: syn::Type,
        // boxed to reduce size difference between enum variations
        field: Box<Field>,
    },
    RemoveModel {
        table_name: String,
        model_ty: syn::Type,
        fields: Vec<Field>,
    },
}

/// Returns whether given [`Field`] is a foreign key to given type.
fn is_field_foreign_key_to(field: &Field, ty: &syn::Type) -> bool {
    foreign_key_for_field(field).is_some_and(|to_model| &to_model == ty)
}

/// Returns the type of the model that the given field is a foreign key to.
/// Returns [`None`] if the field is not a foreign key.
fn foreign_key_for_field(field: &Field) -> Option<syn::Type> {
    match field.foreign_key.clone() {
        None => None,
        Some(foreign_key_spec) => Some(foreign_key_spec.to_model),
    }
}

impl Repr for DynOperation {
    fn repr(&self) -> TokenStream {
        match self {
            Self::CreateModel {
                table_name, fields, ..
            } => {
                let fields = fields.iter().map(Repr::repr).collect::<Vec<_>>();
                quote! {
                    ::cot::db::migrations::Operation::create_model()
                        .table_name(::cot::db::Identifier::new(#table_name))
                        .fields(&[
                            #(#fields,)*
                        ])
                        .build()
                }
            }
            Self::AddField {
                table_name, field, ..
            } => {
                let field = field.repr();
                quote! {
                    ::cot::db::migrations::Operation::add_field()
                        .table_name(::cot::db::Identifier::new(#table_name))
                        .field(#field)
                        .build()
                }
            }
            Self::RemoveField {
                table_name, field, ..
            } => {
                let field = field.repr();
                quote! {
                    ::cot::db::migrations::Operation::remove_field()
                        .table_name(::cot::db::Identifier::new(#table_name))
                        .field(#field)
                        .build()
                }
            }
            Self::RemoveModel {
                table_name, fields, ..
            } => {
                let fields = fields.iter().map(Repr::repr).collect::<Vec<_>>();
                quote! {
                    ::cot::db::migrations::Operation::remove_model()
                        .table_name(::cot::db::Identifier::new(#table_name))
                        .fields(&[
                            #(#fields,)*
                        ])
                        .build()
                }
            }
        }
    }
}

#[derive(Debug)]
struct ParsingError {
    message: String,
    path: PathBuf,
    location: String,
    source: Option<String>,
}

impl ParsingError {
    fn from_darling(message: &str, path: PathBuf, error: &darling::Error) -> Self {
        let message = format!("{message}: {error}");
        let span = error.span();
        let location = format!("{}:{}", span.start().line, span.start().column);

        Self {
            message,
            path,
            location,
            source: span.source_text().clone(),
        }
    }
}

impl Display for ParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(source) = &self.source {
            write!(f, "\n{source}")?;
        }
        write!(f, "\n    at {}:{}", self.path.display(), self.location)?;
        Ok(())
    }
}

impl Error for ParsingError {}

#[cfg(test)]
mod tests {
    use cot_codegen::model::ForeignKeySpec;

    use super::*;

    #[test]
    fn migration_processor_next_migration_name_empty() {
        let migrations = vec![];
        let processor = MigrationProcessor::new(migrations).unwrap();

        let next_migration_name = processor.next_migration_name().unwrap();
        assert_eq!(next_migration_name, "m_0001_initial");
    }

    #[test]
    fn migration_processor_dependencies_empty() {
        let migrations = vec![];
        let processor = MigrationProcessor::new(migrations).unwrap();

        let next_migration_name = processor.base_dependencies();
        assert_eq!(next_migration_name, vec![]);
    }

    #[test]
    fn migration_processor_dependencies_previous() {
        let migrations = vec![Migration {
            app_name: "app1".to_string(),
            name: "m0001_initial".to_string(),
            models: vec![],
        }];
        let processor = MigrationProcessor::new(migrations).unwrap();

        let next_migration_name = processor.base_dependencies();
        assert_eq!(
            next_migration_name,
            vec![DynDependency::Migration {
                app: "app1".to_string(),
                migration: "m0001_initial".to_string(),
            }]
        );
    }

    #[test]
    fn toposort_operations() {
        let mut operations = vec![
            DynOperation::AddField {
                table_name: "table2".to_string(),
                model_ty: parse_quote!(Table2),
                field: Box::new(Field {
                    field_name: format_ident!("field1"),
                    column_name: "field1".to_string(),
                    ty: parse_quote!(i32),
                    auto_value: false,
                    primary_key: false,
                    unique: false,
                    foreign_key: Some(ForeignKeySpec {
                        to_model: parse_quote!(Table1),
                    }),
                }),
            },
            DynOperation::CreateModel {
                table_name: "table1".to_string(),
                model_ty: parse_quote!(Table1),
                fields: vec![],
            },
        ];

        GeneratedMigration::toposort_operations(&mut operations);

        assert_eq!(operations.len(), 2);
        if let DynOperation::CreateModel { table_name, .. } = &operations[0] {
            assert_eq!(table_name, "table1");
        } else {
            panic!("Expected CreateModel operation");
        }
        if let DynOperation::AddField { table_name, .. } = &operations[1] {
            assert_eq!(table_name, "table2");
        } else {
            panic!("Expected AddField operation");
        }
    }

    #[test]
    fn remove_cycles() {
        let mut operations = vec![
            DynOperation::CreateModel {
                table_name: "table1".to_string(),
                model_ty: parse_quote!(Table1),
                fields: vec![Field {
                    field_name: format_ident!("field1"),
                    column_name: "field1".to_string(),
                    ty: parse_quote!(ForeignKey<Table2>),
                    auto_value: false,
                    primary_key: false,
                    unique: false,
                    foreign_key: Some(ForeignKeySpec {
                        to_model: parse_quote!(Table2),
                    }),
                }],
            },
            DynOperation::CreateModel {
                table_name: "table2".to_string(),
                model_ty: parse_quote!(Table2),
                fields: vec![Field {
                    field_name: format_ident!("field1"),
                    column_name: "field1".to_string(),
                    ty: parse_quote!(ForeignKey<Table1>),
                    auto_value: false,
                    primary_key: false,
                    unique: false,
                    foreign_key: Some(ForeignKeySpec {
                        to_model: parse_quote!(Table1),
                    }),
                }],
            },
        ];

        GeneratedMigration::remove_cycles(&mut operations);

        assert_eq!(operations.len(), 3);
        if let DynOperation::CreateModel {
            table_name, fields, ..
        } = &operations[0]
        {
            assert_eq!(table_name, "table1");
            assert!(!fields.is_empty());
        } else {
            panic!("Expected CreateModel operation");
        }
        if let DynOperation::CreateModel {
            table_name, fields, ..
        } = &operations[1]
        {
            assert_eq!(table_name, "table2");
            assert!(fields.is_empty());
        } else {
            panic!("Expected CreateModel operation");
        }
        if let DynOperation::AddField { table_name, .. } = &operations[2] {
            assert_eq!(table_name, "table2");
        } else {
            panic!("Expected AddField operation");
        }
    }

    #[test]
    fn remove_dependency() {
        let mut create_model_op = DynOperation::CreateModel {
            table_name: "table1".to_string(),
            model_ty: parse_quote!(Table1),
            fields: vec![Field {
                field_name: format_ident!("field1"),
                column_name: "field1".to_string(),
                ty: parse_quote!(ForeignKey<Table2>),
                auto_value: false,
                primary_key: false,
                unique: false,
                foreign_key: Some(ForeignKeySpec {
                    to_model: parse_quote!(Table2),
                }),
            }],
        };

        let add_field_op = DynOperation::CreateModel {
            table_name: "table2".to_string(),
            model_ty: parse_quote!(Table2),
            fields: vec![],
        };

        let additional_ops =
            GeneratedMigration::remove_dependency(&mut create_model_op, &add_field_op);

        match create_model_op {
            DynOperation::CreateModel { fields, .. } => {
                assert_eq!(fields.len(), 0);
            }
            _ => {
                panic!("Expected from operation not to change type");
            }
        }
        assert_eq!(additional_ops.len(), 1);
        if let DynOperation::AddField { table_name, .. } = &additional_ops[0] {
            assert_eq!(table_name, "table1");
        } else {
            panic!("Expected AddField operation");
        }
    }

    #[test]
    fn get_foreign_key_dependencies_no_foreign_keys() {
        let operations = vec![DynOperation::CreateModel {
            table_name: "table1".to_string(),
            model_ty: parse_quote!(Table1),
            fields: vec![],
        }];

        let external_dependencies = GeneratedMigration::get_foreign_key_dependencies(&operations);
        assert!(external_dependencies.is_empty());
    }

    #[test]
    fn get_foreign_key_dependencies_with_foreign_keys() {
        let operations = vec![DynOperation::CreateModel {
            table_name: "table1".to_string(),
            model_ty: parse_quote!(Table1),
            fields: vec![Field {
                field_name: format_ident!("field1"),
                column_name: "field1".to_string(),
                ty: parse_quote!(ForeignKey<Table2>),
                auto_value: false,
                primary_key: false,
                unique: false,
                foreign_key: Some(ForeignKeySpec {
                    to_model: parse_quote!(crate::Table2),
                }),
            }],
        }];

        let external_dependencies = GeneratedMigration::get_foreign_key_dependencies(&operations);
        assert_eq!(external_dependencies.len(), 1);
        assert_eq!(
            external_dependencies[0],
            DynDependency::Model {
                model_type: parse_quote!(crate::Table2),
            }
        );
    }

    #[test]
    fn get_foreign_key_dependencies_with_multiple_foreign_keys() {
        let operations = vec![
            DynOperation::CreateModel {
                table_name: "table1".to_string(),
                model_ty: parse_quote!(Table1),
                fields: vec![Field {
                    field_name: format_ident!("field1"),
                    column_name: "field1".to_string(),
                    ty: parse_quote!(ForeignKey<Table2>),
                    auto_value: false,
                    primary_key: false,
                    unique: false,
                    foreign_key: Some(ForeignKeySpec {
                        to_model: parse_quote!(my_crate::Table2),
                    }),
                }],
            },
            DynOperation::CreateModel {
                table_name: "table3".to_string(),
                model_ty: parse_quote!(Table3),
                fields: vec![Field {
                    field_name: format_ident!("field2"),
                    column_name: "field2".to_string(),
                    ty: parse_quote!(ForeignKey<Table4>),
                    auto_value: false,
                    primary_key: false,
                    unique: false,
                    foreign_key: Some(ForeignKeySpec {
                        to_model: parse_quote!(crate::Table4),
                    }),
                }],
            },
        ];

        let external_dependencies = GeneratedMigration::get_foreign_key_dependencies(&operations);
        assert_eq!(external_dependencies.len(), 2);
        assert!(external_dependencies.contains(&DynDependency::Model {
            model_type: parse_quote!(my_crate::Table2),
        }));
        assert!(external_dependencies.contains(&DynDependency::Model {
            model_type: parse_quote!(crate::Table4),
        }));
    }

    #[test]
    fn make_add_field_operation() {
        let app_model = ModelInSource {
            model_item: parse_quote! {
                struct TestModel {
                    #[model(primary_key)]
                    id: i32,
                    field1: i32,
                }
            },
            model: Model {
                name: format_ident!("TestModel"),
                vis: syn::Visibility::Inherited,
                original_name: "TestModel".to_string(),
                resolved_ty: parse_quote!(TestModel),
                model_type: ModelType::default(),
                table_name: "test_model".to_string(),
                pk_field: Field {
                    field_name: format_ident!("id"),
                    column_name: "id".to_string(),
                    ty: parse_quote!(i32),
                    auto_value: true,
                    primary_key: true,
                    unique: false,
                    foreign_key: None,
                },
                fields: vec![],
            },
        };

        let field = Field {
            field_name: format_ident!("new_field"),
            column_name: "new_field".to_string(),
            ty: parse_quote!(i32),
            auto_value: false,
            primary_key: false,
            unique: false,
            foreign_key: None,
        };

        let operation = MigrationOperationGenerator::make_add_field_operation(&app_model, &field);

        match operation {
            DynOperation::AddField {
                table_name,
                model_ty,
                field: op_field,
            } => {
                assert_eq!(table_name, "test_model");
                assert_eq!(model_ty, parse_quote!(TestModel));
                assert_eq!(op_field.column_name, "new_field");
                assert_eq!(op_field.ty, parse_quote!(i32));
            }
            _ => panic!("Expected AddField operation"),
        }
    }

    #[test]
    fn make_create_model_operation() {
        let app_model = ModelInSource {
            model_item: parse_quote! {
                struct TestModel {
                    #[model(primary_key)]
                    id: i32,
                    field1: i32,
                }
            },
            model: Model {
                name: format_ident!("TestModel"),
                vis: syn::Visibility::Inherited,
                original_name: "TestModel".to_string(),
                resolved_ty: parse_quote!(TestModel),
                model_type: ModelType::default(),
                table_name: "test_model".to_string(),
                pk_field: Field {
                    field_name: format_ident!("id"),
                    column_name: "id".to_string(),
                    ty: parse_quote!(i32),
                    auto_value: true,
                    primary_key: true,
                    unique: false,
                    foreign_key: None,
                },
                fields: vec![Field {
                    field_name: format_ident!("field1"),
                    column_name: "field1".to_string(),
                    ty: parse_quote!(i32),
                    auto_value: false,
                    primary_key: false,
                    unique: false,
                    foreign_key: None,
                }],
            },
        };

        let operation = MigrationOperationGenerator::make_create_model_operation(&app_model);

        match operation {
            DynOperation::CreateModel {
                table_name,
                model_ty,
                fields,
            } => {
                assert_eq!(table_name, "test_model");
                assert_eq!(model_ty, parse_quote!(TestModel));
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].column_name, "field1");
            }
            _ => panic!("Expected CreateModel operation"),
        }
    }

    #[test]
    fn generate_operations_with_new_model() {
        let app_model = ModelInSource {
            model_item: parse_quote! {
                struct NewModel {
                    #[model(primary_key)]
                    id: i32,
                    name: String,
                }
            },
            model: Model {
                name: format_ident!("NewModel"),
                vis: syn::Visibility::Inherited,
                original_name: "NewModel".to_string(),
                resolved_ty: parse_quote!(NewModel),
                model_type: ModelType::default(),
                table_name: "new_model".to_string(),
                pk_field: Field {
                    field_name: format_ident!("id"),
                    column_name: "id".to_string(),
                    ty: parse_quote!(i32),
                    auto_value: true,
                    primary_key: true,
                    unique: false,
                    foreign_key: None,
                },
                fields: vec![Field {
                    field_name: format_ident!("name"),
                    column_name: "name".to_string(),
                    ty: parse_quote!(String),
                    auto_value: false,
                    primary_key: false,
                    unique: false,
                    foreign_key: None,
                }],
            },
        };

        let app_models = vec![app_model.clone()];
        let migration_models = vec![];

        let (modified_models, operations) =
            MigrationGenerator::generate_operations(&app_models, &migration_models);

        assert_eq!(modified_models.len(), 1);
        assert_eq!(operations.len(), 1);

        match &operations[0] {
            DynOperation::CreateModel { table_name, .. } => {
                assert_eq!(table_name, "new_model");
            }
            _ => panic!("Expected CreateModel operation"),
        }
    }

    #[test]
    fn make_remove_model_operation() {
        let migration_model = ModelInSource {
            model_item: parse_quote! {
                struct UserModel {
                    #[model(primary_key)]
                    id: i32,
                    name: String,
                }
            },
            model: Model {
                name: format_ident!("UserModel"),
                vis: syn::Visibility::Inherited,
                original_name: "UserModel".to_string(),
                resolved_ty: parse_quote!(UserModel),
                model_type: ModelType::default(),
                table_name: "user_model".to_string(),
                pk_field: Field {
                    field_name: format_ident!("id"),
                    column_name: "id".to_string(),
                    ty: parse_quote!(i32),
                    auto_value: true,
                    primary_key: true,
                    unique: false,
                    foreign_key: None,
                },
                fields: vec![Field {
                    field_name: format_ident!("name"),
                    column_name: "name".to_string(),
                    ty: parse_quote!(String),
                    auto_value: false,
                    primary_key: false,
                    unique: false,
                    foreign_key: None,
                }],
            },
        };

        let operation = MigrationOperationGenerator::make_remove_model_operation(&migration_model);

        match &operation {
            DynOperation::RemoveModel {
                table_name, fields, ..
            } => {
                assert_eq!(table_name, "user_model");
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].column_name, "name");
            }
            _ => panic!("Expected DynOperation::RemoveModel"),
        }
    }
    #[test]
    fn make_remove_field_operation() {
        let migration_model = ModelInSource {
            model_item: parse_quote! {
                struct UserModel {
                    #[model(primary_key)]
                    id: i32,
                    name: String,
                    email: String,
                }
            },
            model: Model {
                name: format_ident!("UserModel"),
                vis: syn::Visibility::Inherited,
                original_name: "UserModel".to_string(),
                resolved_ty: parse_quote!(UserModel),
                model_type: ModelType::default(),
                table_name: "user_model".to_string(),
                pk_field: Field {
                    field_name: format_ident!("id"),
                    column_name: "id".to_string(),
                    ty: parse_quote!(i32),
                    auto_value: true,
                    primary_key: true,
                    unique: false,
                    foreign_key: None,
                },
                fields: vec![
                    Field {
                        field_name: format_ident!("name"),
                        column_name: "name".to_string(),
                        ty: parse_quote!(String),
                        auto_value: false,
                        primary_key: false,
                        unique: false,
                        foreign_key: None,
                    },
                    Field {
                        field_name: format_ident!("email"),
                        column_name: "email".to_string(),
                        ty: parse_quote!(String),
                        auto_value: false,
                        primary_key: false,
                        unique: false,
                        foreign_key: None,
                    },
                ],
            },
        };

        let field = &migration_model.model.fields[1];
        let operation =
            MigrationOperationGenerator::make_remove_field_operation(&migration_model, field);

        match &operation {
            DynOperation::RemoveField {
                table_name,
                model_ty,
                field,
            } => {
                assert_eq!(table_name, "user_model");
                assert_eq!(model_ty, &parse_quote!(UserModel));
                assert_eq!(field.column_name, "email");
                assert_eq!(field.ty, parse_quote!(String));
            }
            _ => panic!("Expected DynOperation::RemoveField"),
        }
    }
    #[test]
    fn generate_operations_with_removed_model() {
        let app_models = vec![];

        let migration_model = ModelInSource {
            model_item: parse_quote! {
                struct UserModel {
                    #[model(primary_key)]
                    id: i32,
                    name: String,
                }
            },
            model: Model {
                name: format_ident!("UserModel"),
                vis: syn::Visibility::Inherited,
                original_name: "UserModel".to_string(),
                resolved_ty: parse_quote!(UserModel),
                model_type: ModelType::default(),
                table_name: "user_model".to_string(),
                pk_field: Field {
                    field_name: format_ident!("id"),
                    column_name: "id".to_string(),
                    ty: parse_quote!(i32),
                    auto_value: true,
                    primary_key: true,
                    unique: false,
                    foreign_key: None,
                },
                fields: vec![Field {
                    field_name: format_ident!("name"),
                    column_name: "name".to_string(),
                    ty: parse_quote!(String),
                    auto_value: false,
                    primary_key: false,
                    unique: false,
                    foreign_key: None,
                }],
            },
        };

        let migration_models = vec![migration_model.clone()];

        let (_modified_models, operations) =
            MigrationGenerator::generate_operations(&app_models, &migration_models);

        assert_eq!(operations.len(), 1);

        match &operations[0] {
            DynOperation::RemoveModel { table_name, .. } => {
                assert_eq!(table_name, "user_model");
            }
            _ => panic!("Expected DynOperation::RemoveModel"),
        }
    }

    #[test]
    fn generate_operations_with_modified_model() {
        let app_model = ModelInSource {
            model_item: parse_quote! {
                struct UserModel {
                    #[model(primary_key)]
                    id: i32,
                    name: String,
                    email: String,
                }
            },
            model: Model {
                name: format_ident!("UserModel"),
                vis: syn::Visibility::Inherited,
                original_name: "UserModel".to_string(),
                resolved_ty: parse_quote!(UserModel),
                model_type: ModelType::default(),
                table_name: "user_model".to_string(),
                pk_field: Field {
                    field_name: format_ident!("id"),
                    column_name: "id".to_string(),
                    ty: parse_quote!(i32),
                    auto_value: true,
                    primary_key: true,
                    unique: false,
                    foreign_key: None,
                },
                fields: vec![
                    Field {
                        field_name: format_ident!("name"),
                        column_name: "name".to_string(),
                        ty: parse_quote!(String),
                        auto_value: false,
                        primary_key: false,
                        unique: false,
                        foreign_key: None,
                    },
                    Field {
                        field_name: format_ident!("email"),
                        column_name: "email".to_string(),
                        ty: parse_quote!(String),
                        auto_value: false,
                        primary_key: false,
                        unique: false,
                        foreign_key: None,
                    },
                ],
            },
        };

        let migration_model = ModelInSource {
            model_item: parse_quote! {
                struct UserModel {
                    #[model(primary_key)]
                    id: i32,
                    name: String,
                }
            },
            model: Model {
                name: format_ident!("UserModel"),
                vis: syn::Visibility::Inherited,
                original_name: "UserModel".to_string(),
                resolved_ty: parse_quote!(UserModel),
                model_type: ModelType::default(),
                table_name: "user_model".to_string(),
                pk_field: Field {
                    field_name: format_ident!("id"),
                    column_name: "id".to_string(),
                    ty: parse_quote!(i32),
                    auto_value: true,
                    primary_key: true,
                    unique: false,
                    foreign_key: None,
                },
                fields: vec![Field {
                    field_name: format_ident!("name"),
                    column_name: "name".to_string(),
                    ty: parse_quote!(String),
                    auto_value: false,
                    primary_key: false,
                    unique: false,
                    foreign_key: None,
                }],
            },
        };

        let app_models = vec![app_model.clone()];
        let migration_models = vec![migration_model.clone()];

        let (modified_models, operations) =
            MigrationGenerator::generate_operations(&app_models, &migration_models);

        assert_eq!(modified_models.len(), 1);
        assert!(!operations.is_empty(), "Expected at least one operation");

        let has_add_field = operations.iter().any(|op| match op {
            DynOperation::AddField { field, .. } => field.column_name == "email",
            _ => false,
        });

        assert!(has_add_field, "Expected an AddField operation for 'email'");
    }
    #[test]
    fn repr_for_remove_field_operation() {
        let op = DynOperation::RemoveField {
            table_name: "test_table".to_string(),
            model_ty: parse_quote!(TestModel),
            field: Box::new(Field {
                field_name: format_ident!("test_field"),
                column_name: "test_field".to_string(),
                ty: parse_quote!(String),
                auto_value: false,
                primary_key: false,
                unique: false,
                foreign_key: None,
            }),
        };

        let tokens = op.repr();
        let tokens_str = tokens.to_string();

        assert!(
            tokens_str.contains("remove_field"),
            "Should call remove_field() but got: {tokens_str}"
        );
        assert!(
            tokens_str.contains("table_name"),
            "Should call table_name() but got: {tokens_str}"
        );
        assert!(
            tokens_str.contains("field"),
            "Should call field() but got: {tokens_str}"
        );
        assert!(
            tokens_str.contains("build"),
            "Should call build() but got: {tokens_str}"
        );
    }
    #[test]
    fn generate_operations_with_removed_field() {
        let app_model = ModelInSource {
            model_item: parse_quote! {
                struct UserModel {
                    #[model(primary_key)]
                    id: i32,
                    name: String,
                }
            },
            model: Model {
                name: format_ident!("UserModel"),
                vis: syn::Visibility::Inherited,
                original_name: "UserModel".to_string(),
                resolved_ty: parse_quote!(UserModel),
                model_type: ModelType::default(),
                table_name: "user_model".to_string(),
                pk_field: Field {
                    field_name: format_ident!("id"),
                    column_name: "id".to_string(),
                    ty: parse_quote!(i32),
                    auto_value: true,
                    primary_key: true,
                    unique: false,
                    foreign_key: None,
                },
                fields: vec![Field {
                    field_name: format_ident!("name"),
                    column_name: "name".to_string(),
                    ty: parse_quote!(String),
                    auto_value: false,
                    primary_key: false,
                    unique: false,
                    foreign_key: None,
                }],
            },
        };

        let migration_model = ModelInSource {
            model_item: parse_quote! {
                struct UserModel {
                    #[model(primary_key)]
                    id: i32,
                    name: String,
                    email: String,
                }
            },
            model: Model {
                name: format_ident!("UserModel"),
                vis: syn::Visibility::Inherited,
                original_name: "UserModel".to_string(),
                resolved_ty: parse_quote!(UserModel),
                model_type: ModelType::default(),
                table_name: "user_model".to_string(),
                pk_field: Field {
                    field_name: format_ident!("id"),
                    column_name: "id".to_string(),
                    ty: parse_quote!(i32),
                    auto_value: true,
                    primary_key: true,
                    unique: false,
                    foreign_key: None,
                },
                fields: vec![
                    Field {
                        field_name: format_ident!("name"),
                        column_name: "name".to_string(),
                        ty: parse_quote!(String),
                        auto_value: false,
                        primary_key: false,
                        unique: false,
                        foreign_key: None,
                    },
                    Field {
                        field_name: format_ident!("email"),
                        column_name: "email".to_string(),
                        ty: parse_quote!(String),
                        auto_value: false,
                        primary_key: false,
                        unique: false,
                        foreign_key: None,
                    },
                ],
            },
        };

        let app_models = vec![app_model.clone()];
        let migration_models = vec![migration_model.clone()];

        let (modified_models, operations) =
            MigrationGenerator::generate_operations(&app_models, &migration_models);

        assert_eq!(modified_models.len(), 1);
        assert!(!operations.is_empty(), "Expected at least one operation");

        let has_remove_field = operations.iter().any(|op| match op {
            DynOperation::RemoveField { field, .. } => field.column_name == "email",
            _ => false,
        });

        assert!(
            has_remove_field,
            "Expected a RemoveField operation for 'email'"
        );
    }
    #[test]
    fn get_migration_list() {
        let tempdir = tempfile::tempdir().unwrap();
        let migrations_dir = tempdir.path().join("migrations");
        std::fs::create_dir(&migrations_dir).unwrap();

        File::create(migrations_dir.join("m_0001_initial.rs")).unwrap();
        File::create(migrations_dir.join("m_0002_auto.rs")).unwrap();
        File::create(migrations_dir.join("dummy.rs")).unwrap();
        File::create(migrations_dir.join("m_0003_not_rust_file.txt")).unwrap();

        let migration_list = MigrationGenerator::get_migration_list(&migrations_dir).unwrap();
        assert_eq!(
            migration_list.len(),
            2,
            "Migration list: {migration_list:?}"
        );
        assert!(migration_list.contains(&"m_0001_initial".to_string()));
        assert!(migration_list.contains(&"m_0002_auto".to_string()));
    }

    #[test]
    fn get_migration_module_contents() {
        let contents = MigrationGenerator::get_migration_module_contents(&[
            "m_0001_initial".to_string(),
            "m_0002_auto".to_string(),
        ]);

        let expected = quote! {
            pub mod m_0001_initial;
            pub mod m_0002_auto;

            /// The list of migrations for current app.
            pub const MIGRATIONS: &[&::cot::db::migrations::SyncDynMigration] = &[
                &m_0001_initial::Migration,
                &m_0002_auto::Migration
            ];
        };

        assert_eq!(contents.to_string(), expected.to_string());
    }

    #[test]
    fn parse_file() {
        let file_name = "main.rs";
        let file_content = r#"
            fn main() {
                println!("Hello, world!");
            }
        "#;

        let parsed = SourceFile::parse(PathBuf::from(file_name), file_content).unwrap();

        assert_eq!(parsed.path, PathBuf::from(file_name));
        assert_eq!(parsed.content.items.len(), 1);
        if let syn::Item::Fn(func) = &parsed.content.items[0] {
            assert_eq!(func.sig.ident.to_string(), "main");
        } else {
            panic!("Expected a function item");
        }
    }
}
