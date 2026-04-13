use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cot_cli::new_project::{CotSource, new_project};
use libtest_mimic::Failed;

pub const COMMON_IMPORTS: &[&str] = &[
    "cot::db::*",
    "cot::request::extractors::*",
    "cot::response::*",
    "cot::html::*",
    "cot::router::*",
    "cot::test::*",
    "cot::project::*",
    "cot::static_files::*",
    "cot::middleware::*",
    "cot::form::*",
    "cot::json::*",
    "cot::cli::*",
    "cot::request::Request",
    "cot::response::Response",
    "cot::*",
    "std::collections::HashMap",
    "std::fmt::Display",
    "serde::{Deserialize, Serialize}",
    "schemars::JsonSchema",
    "askama::filters::HtmlSafe",
    "cot::static_files::StaticFilesMiddleware",
    "cot::project::RootHandler",
    "cot::admin::AdminModel",
    "cot::form::Form",
    "cot::db::Model",
    "cot::project::App",
    "cot::project::Project",
    "cot::cli::CliMetadata",
];

const HTML_MAIN_RS: &str = include_str!("../templates/html_main.rs");
const BASE_HTML: &str = include_str!("../templates/base.html");

pub const RUST_HAS_MAIN_TEST_TYPE: &str = "has_main";

#[derive(Debug)]
pub struct DocTestProject {
    path: PathBuf,
    temp_dir: PathBuf,
}

impl DocTestProject {
    /// Creates a new `DocTestProject` instance.
    ///
    /// # Panics
    ///
    /// Panics if it fails to create the project directory or write files.
    #[must_use]
    pub fn new(temp_dir: PathBuf) -> Self {
        let project_path = temp_dir.join("doc_test");

        if project_path.exists() {
            fs::remove_dir_all(&project_path)
                .expect("failed to clean up existing project directory");
        }

        let cot_test_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let cot_workspace_path = cot_test_path
            .parent()
            .expect("failed to get workspace path");
        let cot_path = cot_workspace_path.join("cot");

        new_project(&project_path, "doc_test", &CotSource::Path(&cot_path))
            .expect("failed to create doc test project");

        // Add extra dependencies and features for tests
        let cargo_toml_path = project_path.join("Cargo.toml");
        let mut cargo_toml =
            fs::read_to_string(&cargo_toml_path).expect("failed to read Cargo.toml");
        cargo_toml = cargo_toml.replace(
            "features = [\"full\"]",
            "features = [\"full\", \"openapi\", \"swagger-ui\"]",
        );
        cargo_toml.push_str("serde = { version = \"1\", features = [\"derive\"] }\n");
        cargo_toml.push_str("schemars = \"0.9\"\n");
        cargo_toml.push_str("askama = \"0.15\"\n");
        cargo_toml.push_str("async-trait = \"0.1\"\n");

        // Add empty workspace info to prevent Cargo from trying to build the entire
        // workspace when running tests
        cargo_toml.push_str("[workspace]\n");

        fs::write(cargo_toml_path, cargo_toml).expect("failed to write Cargo.toml");

        Self {
            path: project_path,
            temp_dir,
        }
    }

    /// Checks the given Rust code block.
    ///
    /// # Errors
    ///
    /// Returns an error if the code fails to compile.
    ///
    /// # Panics
    ///
    /// Panics if it fails to write the code to a file.
    pub fn check_rust(&self, code: &str, test_type: &str) -> Result<(), Failed> {
        self.cleanup_project()?;

        let mut preamble = String::new();
        for &symbol_part in COMMON_IMPORTS {
            let import = format!("use {symbol_part};");
            let symbol_name = symbol_part
                .rsplit_once("::")
                .map_or(symbol_part, |(_, s)| s);

            if symbol_name == "*" {
                // For wildcards, only add if not already present as a wildcard from the same
                // path
                if !code.contains(symbol_part) {
                    preamble.push_str(&import);
                }
            } else {
                // For specific symbols, only add if the symbol is NOT already imported
                let mut found = false;
                for line in code.lines() {
                    if line.starts_with("use ") && line.contains(symbol_name) {
                        found = true;
                        break;
                    }
                }
                if !found {
                    preamble.push_str(&import);
                }
            }
        }

        let final_code = match test_type {
            RUST_HAS_MAIN_TEST_TYPE => code.to_string(),
            _ => Self::wrap_in_main(&preamble, code),
        };

        let main_rs_path = self.path.join("src/main.rs");
        fs::write(main_rs_path, &final_code).map_err(|e| Failed::from(e.to_string()))?;

        self.create_dummy_files()?;
        self.run_cargo_check()
    }

    fn wrap_in_main(preamble: &str, code: &str) -> String {
        format!(
            r"
#![allow(unused_imports, dead_code, unused_variables, unused_mut)]
{preamble}
fn main() {{
    let _ = async {{
        let _result: cot::Result<()> = async {{
            {code}
            Ok::<(), cot::Error>(())
        }}.await;
    }};
}}
"
        )
    }

    /// Checks the given HTML template.
    ///
    /// # Errors
    ///
    /// Returns an error if the template fails to compile.
    ///
    /// # Panics
    ///
    /// Panics if it fails to write the template to a file.
    pub fn check_html(&self, html: &str) -> Result<(), Failed> {
        self.cleanup_project()?;
        let template_path = self.path.join("templates/index.html");
        fs::write(template_path, html).map_err(|e| Failed::from(e.to_string()))?;

        self.create_dummy_files()?;

        let main_rs_path = self.path.join("src/main.rs");
        fs::write(main_rs_path, HTML_MAIN_RS).map_err(|e| Failed::from(e.to_string()))?;

        self.run_cargo_check()
    }

    fn cleanup_project(&self) -> Result<(), Failed> {
        let templates_dir = self.path.join("templates");
        if templates_dir.exists() {
            fs::remove_dir_all(&templates_dir).ok();
        }
        fs::create_dir_all(&templates_dir).map_err(|e| Failed::from(e.to_string()))?;

        let static_dir = self.path.join("static");
        if static_dir.exists() {
            fs::remove_dir_all(&static_dir).ok();
        }
        fs::create_dir_all(&static_dir).map_err(|e| Failed::from(e.to_string()))?;

        Ok(())
    }

    fn create_dummy_files(&self) -> Result<(), Failed> {
        let templates_dir = self.path.join("templates");
        fs::write(templates_dir.join("base.html"), BASE_HTML)
            .map_err(|e| Failed::from(e.to_string()))?;

        let dummy_templates = ["hello.html", "error.html", "500.html", "index.html"];
        for t in dummy_templates {
            let p = templates_dir.join(t);
            if !p.exists() {
                fs::write(p, "dummy content").map_err(|e| Failed::from(e.to_string()))?;
            }
        }

        let static_dir = self.path.join("static");
        fs::create_dir_all(static_dir.join("css")).ok();
        fs::create_dir_all(static_dir.join("images")).ok();
        fs::write(static_dir.join("css/main.css"), "").ok();
        fs::write(static_dir.join("images/logo.png"), "").ok();

        Ok(())
    }

    fn run_cargo_check(&self) -> Result<(), Failed> {
        let target_dir = self.temp_dir.join("doc_test_target");
        fs::create_dir_all(&target_dir).ok();

        let output = cot_cli::test_utils::project_cargo(&self.path)
            .arg("check")
            .env("CARGO_TARGET_DIR", target_dir)
            .output()
            .map_err(|e| Failed::from(e.to_string()))?;

        if !output.status.success() {
            return Err(Failed::from(format!(
                "cargo check failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            )));
        }
        Ok(())
    }
}

static TEST_PROJECT: OnceLock<Mutex<DocTestProject>> = OnceLock::new();

/// Returns a reference to the global `DocTestProject` instance.
///
/// # Panics
///
/// Panics if it fails to initialize the project.
pub fn get_test_project(temp_dir: PathBuf) -> MutexGuard<'static, DocTestProject> {
    TEST_PROJECT
        .get_or_init(|| Mutex::new(DocTestProject::new(temp_dir)))
        .lock()
        .expect("failed to lock test project")
}
