//! Regenerates `docs/configuration.md` from `cot::config::ProjectConfig`'s type
//! definition.
//!
//! Run via `just generate-config-docs`.

use std::path::PathBuf;

fn main() {
    let content = cot_test::config_reference::generate_config_reference();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let docs_path = manifest_dir
        .parent()
        .expect("failed to get workspace path")
        .join("docs")
        .join("configuration.md");

    std::fs::write(&docs_path, content).expect("failed to write docs/configuration.md");
    println!("Wrote {}", docs_path.display());
}
