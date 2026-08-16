use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use tempfile::TempDir;

pub const FROBNICATE_TASK_SOURCE: &str = r#"
struct Frobnicate;

#[async_trait(?Send)]
impl CliTask for Frobnicate {
    fn subcommand(&self) -> Command {
        Command::new("frobnicate")
            .about("Frobnicates the target")
            .arg(Arg::new("target").required(true).help("What to frobnicate"))
            .arg(Arg::new("intensity").long("intensity").help("How hard to frobnicate"))
            .arg(
                Arg::new("build")
                    .long("build")
                    .action(ArgAction::SetTrue)
                    .help("Simulated flag colliding with cot-cli's own --build"),
            )
    }

    async fn execute(
        &mut self,
        matches: &ArgMatches,
        _bootstrapper: Bootstrapper<WithConfig>,
    ) -> cot::Result<()> {
        let target = matches.get_one::<String>("target").expect("required");
        println!("frobnicating {target}");
        if matches.get_flag("build") {
            println!("(received forwarded --build flag)");
        }
        Ok(())
    }
}
"#;

pub const FROBNICATE_REGISTER: &str = "cli.add_task(Frobnicate);";

pub const GROUPED_TASK_SOURCE: &str = r#"
struct SubA;

#[async_trait(?Send)]
impl CliTask for SubA {
    fn subcommand(&self) -> Command {
        Command::new("sub-a").about("Fixture sub-task A")
    }

    async fn execute(
        &mut self,
        _matches: &ArgMatches,
        _bootstrapper: Bootstrapper<WithConfig>,
    ) -> cot::Result<()> {
        println!("ran sub-a");
        Ok(())
    }
}
"#;

pub const GROUPED_REGISTER: &str = r#"
        let mut group = cot::cli::CliTaskGroup::new("fixture-group").about("Fixture task group");
        group.add_task(SubA);
        cli.add_task(group);
"#;

fn workspace() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("cot-cli should be in a workspace")
            .to_path_buf()
    })
}

fn cot_crate_path() -> PathBuf {
    workspace().join("cot")
}

fn workspace_target_dir() -> PathBuf {
    workspace().join("target")
}

fn default_main_rs(
    project_name: &str,
    extra_code: &str,
    register_calls: &[String],
    apps: &[CotApp],
) -> String {
    let struct_name = to_pascal_case(project_name);
    let register_tasks_body = register_calls.join("\n\t\t");
    let app_definitions = apps
        .iter()
        .map(CotApp::render)
        .collect::<Vec<_>>()
        .join("\n");

    let register_apps_body = apps
        .iter()
        .map(CotApp::render_registration)
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r"mod migrations;

use cot::Project;
use cot::Bootstrapper;
use cot::db::{{Auto, Model, model}};
use cot::cli::{{Cli, CliMetadata, CliTask}};
use cot::cli::clap::{{Arg, ArgAction, ArgMatches, Command}};
use cot::config::ProjectConfig;
use cot::project::{{AppBuilder, RegisterAppsContext, WithConfig}};
use async_trait::async_trait;

#[model]
#[derive(Debug, Clone)]
struct DefaultTestModel {{
    #[model(primary_key)]
    id: Auto<i32>,
    title: String,
}}

{app_definitions}

{extra_code}

struct {struct_name}Project;

impl Project for {struct_name}Project {{
    fn cli_metadata(&self) -> CliMetadata {{
        cot::cli::metadata!()
    }}

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {{
        Ok(ProjectConfig::dev_default())
    }}

    fn register_tasks(&self, cli: &mut Cli) {{
        {register_tasks_body}
    }}

    fn register_apps(
        &self,
        apps: &mut AppBuilder,
        _context: &RegisterAppsContext,
    ) {{
{register_apps_body}
    }}
}}

#[cot::main]
fn main() -> impl Project {{
    {struct_name}Project
}}
"
    )
}

fn default_migrations_rs() -> String {
    r"pub const MIGRATIONS: &[&::cot::db::migrations::SyncDynMigration] = &[];".to_string()
}

fn render_cargo_toml(project_name: &str, features: &[String], extra: &str) -> String {
    let features_str = if features.is_empty() {
        r#"["default"]"#.to_owned()
    } else {
        format!(
            "[{}]",
            features
                .iter()
                .map(|f| format!(r#""{f}""#))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    // normalize path separator
    let cot_path = cot_crate_path().display().to_string().replace('\\', "/");

    format!(
        r#"[package]
name = "{project_name}"
version = "0.1.0"
edition = "2024"

[dependencies]
cot = {{ path = "{cot_path}", features = {features_str} }}
async-trait = "0.1"
{extra}
"#,
    )
}

fn unique_project_name() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    // Use process ID + counter so parallel test processes don't collide.
    format!("cot-test-{}-{count}", std::process::id())
}

/// Builder for a generated Cot application.
///
/// The builder mirrors the methods available on Cot's [`cot::App`] trait.
#[derive(Debug, Clone)]
pub struct CotAppBuilder {
    name: String,
    init: Option<String>,
    router: Option<String>,
    migrations: Option<String>,
    admin_model_managers: Option<String>,
    static_files: Option<String>,
}

impl CotAppBuilder {
    /// Creates a new App builder.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            init: None,
            router: None,
            migrations: None,
            admin_model_managers: None,
            static_files: None,
        }
    }

    /// Sets the implementation of `App::init`.
    #[must_use]
    pub fn init(mut self, body: impl Into<String>) -> Self {
        self.init = Some(body.into());
        self
    }

    /// Sets the implementation of `App::router`.
    #[must_use]
    pub fn router(mut self, code_block: impl Into<String>) -> Self {
        self.router = Some(code_block.into());
        self
    }

    /// Sets the implementation of `App::migrations`.
    #[must_use]
    pub fn migrations(mut self, code_block: impl Into<String>) -> Self {
        self.migrations = Some(code_block.into());
        self
    }

    /// Sets the implementation of `App::admin_model_managers`.
    #[must_use]
    pub fn admin_model_managers(mut self, code_block: impl Into<String>) -> Self {
        self.admin_model_managers = Some(code_block.into());
        self
    }

    /// Sets the implementation of `App::static_files`.
    #[must_use]
    pub fn static_files(mut self, code_block: impl Into<String>) -> Self {
        self.static_files = Some(code_block.into());
        self
    }

    /// Builds the application definition.
    ///
    /// The returned `CotApp` is what gets registered with a project builder.
    #[must_use]
    pub fn build(self) -> CotApp {
        assert!(!self.name.trim().is_empty(), "Cot app name cannot be empty");

        CotApp {
            name: self.name,
            init: self.init,
            router: self.router,
            migrations: self.migrations,
            admin_model_managers: self.admin_model_managers,
            static_files: self.static_files,
        }
    }
}

/// A fully-built generated Cot App
#[derive(Debug, Clone)]
pub struct CotApp {
    name: String,
    init: Option<String>,
    router: Option<String>,
    migrations: Option<String>,
    admin_model_managers: Option<String>,
    static_files: Option<String>,
}

impl CotApp {
    /// Returns the app's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Render this app as Rust source implementing `cot::App`.
    #[must_use]
    pub fn render(&self) -> String {
        let struct_name = format!("{}App", to_pascal_case(&self.name));

        let init = self.render_init();
        let router = self.render_router();
        let migrations = self.render_migrations();
        let admin_model_managers = self.render_admin_model_managers();
        let static_files = self.render_static_files();

        format!(
            r"
struct {struct_name};

#[async_trait]
impl cot::App for {struct_name} {{
    fn name(&self) -> &str {{
        {name:?}
    }}

{init}

{router}

{migrations}

{admin_model_managers}

{static_files}
}}
",
            name = self.name,
        )
    }

    fn render_init(&self) -> String {
        match &self.init {
            Some(body) => format!(
                r"    async fn init(
        &self,
        _context: &mut cot::project::ProjectContext,
    ) -> cot::Result<()> {{
        {body}
    }}"
            ),

            None => r"    async fn init(
        &self,
        _context: &mut cot::project::ProjectContext,
    ) -> cot::Result<()> {
        Ok(())
    }"
            .to_owned(),
        }
    }

    fn render_router(&self) -> String {
        match &self.router {
            Some(code_block) => {
                format!(
                    r"    fn router(&self) -> cot::router::Router {{
        {code_block}
    }}"
                )
            }

            None => r"    fn router(&self) -> cot::router::Router {
        cot::router::Router::empty()
    }"
            .to_owned(),
        }
    }

    fn render_migrations(&self) -> String {
        match &self.migrations {
            Some(code_block) => {
                format!(
                    r#"    #[cfg(feature = "db")]
    fn migrations(&self) -> Vec<Box<cot::db::migrations::SyncDynMigration>> {{
        {code_block}
    }}"#
                )
            }

            None => r#"    #[cfg(feature = "db")]
    fn migrations(&self) -> Vec<Box<cot::db::migrations::SyncDynMigration>> {
        vec![]
    }"#
            .to_owned(),
        }
    }

    fn render_admin_model_managers(&self) -> String {
        match &self.admin_model_managers {
            Some(code_block) => {
                format!(
                    r"    fn admin_model_managers(&self) -> Vec<Box<dyn cot::admin::AdminModelManager>> {{
        {code_block}
    }}"
                )
            }

            None => r"    fn admin_model_managers(&self) -> Vec<Box<dyn cot::admin::AdminModelManager>> {
        vec![]
    }"
                .to_owned(),
        }
    }

    fn render_static_files(&self) -> String {
        match &self.static_files {
            Some(code_block) => {
                format!(
                    r"    fn static_files(&self) -> Vec<cot::static_files::StaticFile> {{
        {code_block}
    }}"
                )
            }

            None => r"    fn static_files(&self) -> Vec<cot::static_files::StaticFile> {
        vec![]
    }"
            .to_owned(),
        }
    }

    /// Returns the code string used to register this app with the generated
    /// project.
    #[must_use]
    pub fn render_registration(&self) -> String {
        let struct_name = format!("{}App", to_pascal_case(&self.name));

        format!("\t\tapps.register({struct_name});")
    }
}

#[derive(Debug)]
pub struct CotProjectBuilder {
    project_name: String,
    cot_binary: PathBuf,
    features: Vec<String>,
    main_rs: Option<String>,
    migrations_rs: Option<String>,
    extra_files: Vec<(PathBuf, String)>,
    extra_cargo_toml: String,
    extra_code: String,
    register_calls: Vec<String>,
    apps: Vec<CotApp>,
}

impl CotProjectBuilder {
    #[must_use]
    pub fn new(cot_binary: PathBuf) -> Self {
        Self {
            project_name: unique_project_name(),
            features: Vec::new(),
            main_rs: None,
            migrations_rs: None,
            extra_files: Vec::new(),
            extra_cargo_toml: String::new(),
            extra_code: String::new(),
            register_calls: Vec::new(),
            apps: Vec::new(),
            cot_binary,
        }
    }

    #[must_use]
    pub fn project_name(mut self, name: impl Into<String>) -> Self {
        self.project_name = name.into();
        self
    }

    #[must_use]
    pub fn features(mut self, features: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.features = features.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn main_rs(mut self, content: impl Into<String>) -> Self {
        self.main_rs = Some(content.into());
        self
    }

    #[must_use]
    pub fn migrations_rs(mut self, content: impl Into<String>) -> Self {
        self.migrations_rs = Some(content.into());
        self
    }

    /// Append raw TOML to the generated `Cargo.toml`.
    #[must_use]
    pub fn cargo_toml_extra(mut self, toml: impl Into<String>) -> Self {
        self.extra_cargo_toml = toml.into();
        self
    }

    /// Insert raw Rust code at the top level of the generated `main.rs`,
    /// above the `Project` impl.
    #[must_use]
    pub fn extra_code(mut self, code: impl Into<String>) -> Self {
        self.extra_code.push_str(&code.into());
        self.extra_code.push('\n');
        self
    }

    /// Add a code block`Project::register_tasks` body.
    #[must_use]
    pub fn register_task(mut self, code_block: impl Into<String>) -> Self {
        self.register_calls.push(code_block.into());
        self
    }

    /// Register an already-built app with this project.
    #[must_use]
    pub fn app(mut self, app: CotApp) -> Self {
        self.apps.push(app);
        self
    }

    /// Register multiple already-built apps with this project.
    #[must_use]
    pub fn apps(mut self, apps: impl IntoIterator<Item = CotApp>) -> Self {
        self.apps.extend(apps);
        self
    }

    /// Add a file to the project, relative to the project root.
    #[must_use]
    pub fn with_file(
        mut self,
        relative_path: impl Into<PathBuf>,
        content: impl Into<String>,
    ) -> Self {
        self.extra_files
            .push((relative_path.into(), content.into()));
        self
    }

    /// Write all project files to a temporary directory.
    ///
    /// Returns a [`CotProject`] that can be used to run commands which
    /// don't require a compiled binary (e.g. `cot migration list`), or can
    /// be compiled via [`CotProject::compile`].
    pub fn build(self) -> Result<CotProject> {
        let tempdir = TempDir::with_prefix("cot-test-harness-")
            .context("failed to create temporary directory for test project")?;

        let project_dir = tempdir.path().join(&self.project_name);
        std::fs::create_dir_all(project_dir.join("src"))
            .context("failed to create project src/ directory")?;

        std::fs::write(
            project_dir.join("Cargo.toml"),
            render_cargo_toml(&self.project_name, &self.features, &self.extra_cargo_toml),
        )
        .context("failed to write Cargo.toml")?;

        let main_rs = self.main_rs.clone().unwrap_or_else(|| {
            default_main_rs(
                &self.project_name,
                &self.extra_code,
                &self.register_calls,
                &self.apps,
            )
        });
        std::fs::write(project_dir.join("src").join("main.rs"), main_rs)
            .context("failed to write src/main.rs")?;

        let migrations_rs = self
            .migrations_rs
            .clone()
            .unwrap_or_else(default_migrations_rs);
        std::fs::write(project_dir.join("src").join("migrations.rs"), migrations_rs)
            .context("failed to write src/migrations.rs")?;

        for (rel, content) in &self.extra_files {
            let abs = project_dir.join(rel);
            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create directory for {}", rel.display()))?;
            }
            std::fs::write(&abs, content)
                .with_context(|| format!("failed to write {}", rel.display()))?;
        }

        Ok(CotProject {
            _tempdir: tempdir,
            project_dir,
            project_name: self.project_name,
            cot_binary: self.cot_binary,
        })
    }
}

/// A temporary Cot project with all files written to disk, but no binary built.
///
/// Suitable for testing CLI commands that operate on source code
///
/// Call [`CotProject::compile`] to build the binary and unlock proxy
/// command testing.
#[derive(Debug)]
pub struct CotProject {
    _tempdir: TempDir,
    project_dir: PathBuf,
    project_name: String,
    cot_binary: PathBuf,
}

impl CotProject {
    /// The absolute path to the project root directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.project_dir
    }

    /// The project name (also the Cargo package name and binary name).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.project_name
    }

    /// Build a `cot` CLI command configured to run in this project's directory.
    ///
    /// Uses the test binary (respects `COT_CLI_TEST_CMD`) and does not require
    /// a compiled project binary.
    #[must_use]
    pub fn cot_cmd(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.cot_binary);
        cmd.current_dir(&self.project_dir);
        cmd.args(args);
        cmd
    }

    /// Build a raw `cargo` command configured to run in this project's
    /// directory.
    ///
    /// The `CARGO_TARGET_DIR` is set to the workspace target so dependencies
    /// are shared across all test project builds.
    #[must_use]
    pub fn cargo_cmd(&self, subcommand: &str, args: &[&str]) -> Command {
        let mut cmd = cargo_bin_command();
        cmd.current_dir(&self.project_dir)
            .env("CARGO_TARGET_DIR", workspace_target_dir())
            .arg(subcommand)
            .args(args);
        cmd
    }

    /// Compile the project binary in debug mode.
    pub fn compile(self) -> Result<CompiledCotProject> {
        self.compile_inner(false)
    }

    /// Compile the project binary in release mode.
    pub fn compile_release(self) -> Result<CompiledCotProject> {
        self.compile_inner(true)
    }

    fn compile_inner(self, release: bool) -> Result<CompiledCotProject> {
        let mut extra_args = vec![];
        if release {
            extra_args.push("--release");
        }

        let status = self
            .cargo_cmd("build", &extra_args)
            .status()
            .context("failed to spawn `cargo build`")?;

        if !status.success() {
            bail!(
                "`cargo build` failed for project `{}` at `{}`",
                self.project_name,
                self.project_dir.display()
            );
        }

        let profile = if release { "release" } else { "debug" };
        let binary_name = platform_binary_name(&self.project_name);

        // The binary was compiled into the workspace target dir.
        let workspace_binary = workspace_target_dir().join(profile).join(&binary_name);

        if !workspace_binary.exists() {
            bail!(
                "expected compiled binary at `{}` but it was not found",
                workspace_binary.display()
            );
        }

        // Bridge the binary into the project's own target tree so that
        // `cot-cli`'s `resolve_target_dir` can find it.
        // On Unix we create a symlink, on Windows we copy.
        let project_target_dir = self.project_dir.join("target").join(profile);
        std::fs::create_dir_all(&project_target_dir)
            .context("failed to create project target directory")?;

        let project_binary = project_target_dir.join(&binary_name);
        link_or_copy(&workspace_binary, &project_binary)
            .context("failed to link binary into project target dir")?;

        Ok(CompiledCotProject {
            inner: self,
            binary_path: project_binary,
            release,
        })
    }
}

/// A temporary Cot project with a compiled binary.
#[derive(Debug)]
pub struct CompiledCotProject {
    inner: CotProject,
    binary_path: PathBuf,
    release: bool,
}

impl CompiledCotProject {
    /// The absolute path to the project root directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    /// The project name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.inner.name()
    }

    /// The absolute path to the compiled binary.
    #[must_use]
    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    /// Whether this is a release build.
    #[must_use]
    pub fn is_release(&self) -> bool {
        self.release
    }

    /// Build a `cot` CLI proxy command configured to run in this project's
    /// directory.
    ///
    /// Automatically appends `--release` if the project was compiled in release
    /// mode so `cot-cli` resolves the correct binary.
    #[must_use]
    pub fn cot_cmd(&self, args: &[&str]) -> Command {
        let mut cmd = self.inner.cot_cmd(args);
        if self.release {
            cmd.arg("--release");
        }
        cmd
    }

    /// Build a `cot` CLI command without any automatic flags.
    ///
    /// Use this when you want to control `--release` manually or test
    /// the error path where the wrong profile binary is specified.
    #[must_use]
    pub fn cot_cmd_raw(&self, args: &[&str]) -> Command {
        self.inner.cot_cmd(args)
    }

    /// Run the project binary directly, bypassing the `cot` CLI proxy.
    ///
    /// Useful for verifying that the binary itself behaves correctly,
    /// independent of proxy machinery.
    #[must_use]
    pub fn binary_cmd(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.binary_path);
        cmd.current_dir(self.path()).args(args);
        cmd
    }

    /// Build a `cargo` command in the project directory.
    #[must_use]
    pub fn cargo_cmd(&self, subcommand: &str, args: &[&str]) -> Command {
        self.inner.cargo_cmd(subcommand, args)
    }
}

/// A lazily-compiled standard project cot project.
///
/// Compiling the same project for every test function would be prohibitively
/// slow. For tests that don't need a custom project structure, use this
/// instead.
///
/// # Examples
///
/// ```
/// # use cot_cli::test_harness::standard_project;
/// let project = standard_project().unwrap();
/// let output = project.cot_cmd(&["check"]).output().unwrap();
/// ```
pub fn standard_project(cot_binary: PathBuf) -> Result<&'static CompiledCotProject> {
    static PROJECT: OnceLock<CompiledCotProject> = OnceLock::new();
    static ERROR: OnceLock<String> = OnceLock::new();

    if let Some(err) = ERROR.get() {
        bail!("standard project failed to compile: {err}");
    }

    if let Some(proj) = PROJECT.get() {
        return Ok(proj);
    }

    let extra_code = format!("{FROBNICATE_TASK_SOURCE}\n{GROUPED_TASK_SOURCE}");

    let standard_app = CotAppBuilder::new("cot_test_standard")
        .migrations("cot::db::migrations::wrap_migrations(migrations::MIGRATIONS)")
        .build();

    match CotProjectBuilder::new(cot_binary)
        .project_name("cot_test_standard")
        .app(standard_app)
        .extra_code(extra_code)
        .register_task(FROBNICATE_REGISTER)
        .register_task(GROUPED_REGISTER)
        .build()
        .and_then(CotProject::compile)
    {
        Ok(proj) => {
            let _ = PROJECT.set(proj);
            Ok(PROJECT.get().unwrap())
        }

        Err(e) => {
            let msg = format!("{e:#}");
            let _ = ERROR.set(msg.clone());
            bail!("standard project failed to compile: {msg}");
        }
    }
}

fn cargo_bin_command() -> Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(cargo);
    // Strip RUSTFLAGS that may have been set by the outer cargo invocation
    // (e.g. instrument-coverage flags), they may conflict with the inner build.
    cmd.env_remove("RUSTFLAGS").env("CARGO_INCREMENTAL", "0");
    cmd
}

fn platform_binary_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn link_or_copy(src: &Path, dst: &Path) -> Result<()> {
    // Remove stale link/copy from a previous test run.
    if dst.exists() || dst.symlink_metadata().is_ok() {
        std::fs::remove_file(dst).context("failed to remove stale binary")?;
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dst)
            .with_context(|| format!("failed to symlink {} → {}", src.display(), dst.display()))
    }

    #[cfg(not(unix))]
    {
        std::fs::copy(src, dst)
            .with_context(|| format!("failed to copy {} → {}", src.display(), dst.display()))
            .map(|_| ())
    }
}

fn to_pascal_case(s: &str) -> String {
    s.split(['-', '_'])
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}
