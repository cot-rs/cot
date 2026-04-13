use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use comrak::arena_tree::NodeEdge;
use comrak::nodes::NodeValue;
use comrak::{Arena, parse_document};
use cot_test::{RUST_HAS_MAIN_TEST_TYPE, get_test_project};
use libtest_mimic::{Arguments, Failed, Trial};

type TestRunner = fn(&str, &str) -> Result<(), Failed>;

static TEST_RUNNERS: OnceLock<HashMap<(&str, &str), TestRunner>> = OnceLock::new();

fn main() {
    let args = Arguments::from_args();

    let mut test_runners: HashMap<(&str, &str), TestRunner> = HashMap::new();
    test_runners.insert(("rust", DEFAULT_TEST_NAME), test_rust);
    test_runners.insert(("rust", RUST_HAS_MAIN_TEST_TYPE), test_rust);
    test_runners.insert(("toml", DEFAULT_TEST_NAME), test_toml);
    test_runners.insert(("html.j2", DEFAULT_TEST_NAME), test_html);
    TEST_RUNNERS.set(test_runners).unwrap();

    let mut trials = Vec::new();

    let cot_test_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let docs_path = cot_test_path.parent().unwrap().join("docs");

    let md_files = glob::glob(&format!("{}/*.md", docs_path.to_str().unwrap()))
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

const DEFAULT_TEST_NAME: &str = "default";

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
                        (lang, test_config.trim())
                    } else {
                        (code_block.info.as_str(), DEFAULT_TEST_NAME)
                    };

                if let Some(runner) = TEST_RUNNERS.get().unwrap().get(&(lang, test_config)) {
                    let literal = if lang == "rust" {
                        clean_code(&code_block.literal)
                    } else {
                        code_block.literal.clone()
                    };

                    let line = node_data.sourcepos.start.line;
                    let runner = *runner;
                    let file_name = file_name.to_string();
                    let test_config = test_config.to_string();
                    trials.push(Trial::test(
                        format!("{file_name}; line {line}; {lang},{test_config}"),
                        move || runner(&literal, &test_config),
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

fn test_rust(code: &str, test_type: &str) -> Result<(), Failed> {
    let project = get_test_project(get_temp_dir());
    project.check_rust(code, test_type)
}

fn test_toml(code: &str, _test_type: &str) -> Result<(), Failed> {
    cot::config::ProjectConfig::from_toml(code)
        .map_err(|e| Failed::from(format!("could not parse the config: {e}")))?;

    Ok(())
}

fn test_html(code: &str, _test_type: &str) -> Result<(), Failed> {
    let project = get_test_project(get_temp_dir());
    project.check_html(code)
}
