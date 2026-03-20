use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Once, OnceLock};

use comrak::arena_tree::NodeEdge;
use comrak::nodes::NodeValue;
use comrak::{Arena, Options, format_html, parse_document};
use cot_cli::new_project::{CotSource, new_project};
use libtest_mimic::{Arguments, Failed, Trial};

static TEST_RUNNERS: OnceLock<HashMap<(&str, &str), fn(&str) -> Result<(), Failed>>> =
    OnceLock::new();

macro_rules! add_md {
    ($trials:ident, $path:literal) => {
        test_md(
            &mut $trials,
            $path,
            include_str!(concat!("../../docs/", $path)),
        );
    };
}

fn main() {
    let args = Arguments::from_args();

    let mut test_runners = HashMap::new();
    test_runners.insert(
        ("rust", DEFAULT_TEST_NAME),
        test_rust_default as fn(&str) -> Result<(), Failed>,
    );
    test_runners.insert(
        ("toml", DEFAULT_TEST_NAME),
        test_toml_default as fn(&str) -> Result<(), Failed>,
    );
    test_runners.insert(
        ("html.j2", DEFAULT_TEST_NAME),
        test_html_default as fn(&str) -> Result<(), Failed>,
    );
    TEST_RUNNERS.set(test_runners).unwrap();

    let mut trials = Vec::new();
    add_md!(trials, "admin-panel.md");
    add_md!(trials, "caching.md");
    add_md!(trials, "db-models.md");
    add_md!(trials, "error-pages.md");
    add_md!(trials, "framework-comparison.md");
    add_md!(trials, "introduction.md");
    add_md!(trials, "openapi.md");
    add_md!(trials, "sending-emails.md");
    add_md!(trials, "static-files.md");
    add_md!(trials, "templates.md");
    add_md!(trials, "testing.md");
    add_md!(trials, "upgrade-guide.md");
    libtest_mimic::run(&args, trials).exit();
}

const DEFAULT_TEST_NAME: &str = "default";
fn test_md(trials: &mut Vec<Trial>, file_name: &str, file_contents: &str) {
    let arena = Arena::new();

    let mut options = comrak::Options::default();
    options.extension.front_matter_delimiter = Some("---".to_string());

    let root = parse_document(&arena, file_contents, &Options::default());

    for node in root.traverse() {
        if let NodeEdge::Start(node) = node {
            let node_data = node.data.borrow();
            if let NodeValue::CodeBlock(code_block) = &node_data.value {
                let (lang, test_config) =
                    if let Some((lang, test_config)) = code_block.info.split_once(",") {
                        (lang, test_config)
                    } else {
                        (code_block.info.as_str(), DEFAULT_TEST_NAME)
                    };
                let literal = code_block.literal.clone();

                if let Some(runner) = TEST_RUNNERS.get().unwrap().get(&(lang, test_config)) {
                    let line = node_data.sourcepos.start.line;
                    let runner = runner.clone();
                    trials.push(Trial::test(
                        format!("{file_name}; line {line}; {lang},{test_config}"),
                        move || runner(&literal),
                    ));
                }
            }
        }
    }
}

fn test_rust_default(code: &str) -> Result<(), Failed> {
    // let temp_dir = tempfile::tempdir()?;
    // let project_path = temp_dir.path().join("my_project");
    //
    // let cot_cli_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // let cot_workspace_path = cot_cli_path.parent().unwrap().join("cot");
    // new_project(
    //     &project_path,
    //     "my_project",
    //     &CotSource::Path(&cot_workspace_path),
    // )?;
    //
    // let output = cot_cli::test_utils::project_cargo(&project_path)
    //     .arg("run")
    //     .arg("--quiet")
    //     .arg("--")
    //     .arg("check")
    //     .output()?;
    //
    // let status = output.status;
    // let stdout = String::from_utf8_lossy(&output.stdout);
    // let stderr = String::from_utf8_lossy(&output.stderr);
    // assert!(status.success(), "status: {status}, stderr: {stderr}");
    // assert!(
    //     stdout.contains("Success verifying the configuration"),
    //     "status: {status}, stderr: {stderr}"
    // );

    Ok(())
}

fn test_toml_default(code: &str) -> Result<(), Failed> {
    cot::config::ProjectConfig::from_toml(code)?;

    Ok(())
}

fn test_html_default(code: &str) -> Result<(), Failed> {
    Ok(())
}
