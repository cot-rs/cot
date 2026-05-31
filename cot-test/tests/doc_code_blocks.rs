use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use comrak::arena_tree::NodeEdge;
use comrak::nodes::NodeValue;
use comrak::{Arena, parse_document};
use cot_test::{RUST_HAS_MAIN_TEST_TYPE, TestConfig, TestLanguage, get_test_project};
use libtest_mimic::{Arguments, Failed, Trial};

type TestRunner = fn(&str) -> Result<(), Failed>;

static TEST_RUNNERS: OnceLock<HashMap<(TestLanguage, TestConfig), TestRunner>> = OnceLock::new();

fn main() {
    let args = Arguments::from_args();

    let mut test_runners: HashMap<(TestLanguage, TestConfig), TestRunner> = HashMap::new();
    test_runners.insert((TestLanguage::Rust, TestConfig::Default), test_rust_default);
    test_runners.insert(
        (TestLanguage::Rust, TestConfig::HasMain),
        test_rust_with_main,
    );
    test_runners.insert((TestLanguage::Toml, TestConfig::Default), test_toml);
    test_runners.insert(
        (TestLanguage::AskamaTemplate, TestConfig::Default),
        test_html,
    );
    TEST_RUNNERS.set(test_runners).unwrap();

    let mut trials = Vec::new();

    let cot_test_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let docs_path = cot_test_path.parent().unwrap().join("docs");

    let md_files = glob::glob(&format!("{}/**/*.md", docs_path.to_str().unwrap()))
        .expect("failed to glob md files");

    for entry in md_files {
        let path = entry.expect("failed to read glob entry");
        let file_name = path.file_name().unwrap().to_str().unwrap();
        if file_name == "README.md" {
            continue;
        }

        let contents = fs::read_to_string(&path).expect("failed to read md file");
        test_md(&mut trials, file_name, &contents);
    }

    libtest_mimic::run(&args, trials).exit();
}

const DEFAULT_TEST_TYPE: &str = "default";

fn test_md(trials: &mut Vec<Trial>, file_name: &str, file_contents: &str) {
    let arena = Arena::new();

    let mut options = comrak::Options::default();
    options.extension.front_matter_delimiter = Some("---".to_string());

    let root = parse_document(&arena, file_contents, &options);

    for node in root.traverse() {
        if let NodeEdge::Start(node) = node {
            let node_data = node.data.borrow();
            if let NodeValue::CodeBlock(code_block) = &node_data.value {
                let (lang, test_config) =
                    if let Some((lang, test_config)) = code_block.info.split_once(',') {
                        (
                            TestLanguage::try_from(lang),
                            TestConfig::try_from(test_config.trim()).expect("unknown test config"),
                        )
                    } else {
                        (
                            TestLanguage::try_from(code_block.info.as_str()),
                            TestConfig::Default,
                        )
                    };
                let lang = lang.expect("unknown language");

                if let Some(runner) = TEST_RUNNERS.get().unwrap().get(&(lang, test_config)) {
                    let literal = if lang == TestLanguage::Rust {
                        clean_code(&code_block.literal)
                    } else {
                        code_block.literal.clone()
                    };

                    let line = node_data.sourcepos.start.line;
                    let runner = *runner;
                    let file_name = file_name.to_string();
                    trials.push(Trial::test(
                        format!(
                            "{file_name}; line {line}; language {lang:?}; config {test_config:?}"
                        ),
                        move || runner(&literal),
                    ));
                }
            }
        }
    }
}

fn clean_code(code: &str) -> String {
    code.lines()
        .map(|line| {
            if let Some(rest) = line.strip_prefix("# ") {
                rest
            } else if line == "#" {
                ""
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn get_temp_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
}

fn test_rust_default(code: &str) -> Result<(), Failed> {
    test_rust(code, TestConfig::Default)
}

fn test_rust_with_main(code: &str) -> Result<(), Failed> {
    test_rust(code, TestConfig::HasMain)
}

fn test_rust(code: &str, test_config: TestConfig) -> Result<(), Failed> {
    let project = get_test_project(get_temp_dir());
    project.check_rust(code, test_config)
}

fn test_toml(code: &str) -> Result<(), Failed> {
    cot::config::ProjectConfig::from_toml(code)
        .map_err(|e| Failed::from(format!("could not parse the config: {e}")))?;

    Ok(())
}

fn test_html(code: &str) -> Result<(), Failed> {
    let project = get_test_project(get_temp_dir());
    project.check_html(code)
}
