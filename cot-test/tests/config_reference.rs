//! Verifies that `docs/configuration.md` is up to date with
//! `cot::config::ProjectConfig`'s current type definition. If this fails, run
//! `just generate-config-docs` and commit the result.

use std::path::PathBuf;

#[test]
fn config_reference_is_up_to_date() {
    let generated = cot_test::config_reference::generate_config_reference();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let docs_path = manifest_dir
        .parent()
        .expect("failed to get workspace path")
        .join("docs")
        .join("configuration.md");
    let committed = std::fs::read_to_string(&docs_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", docs_path.display()));

    assert_eq!(
        generated, committed,
        "docs/configuration.md is out of date with cot::config::ProjectConfig; \
         run `just generate-config-docs` and commit the result"
    );
}
