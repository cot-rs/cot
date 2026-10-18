//! Compiles the SCSS sources in the `cot` crate into the CSS files that are
//! committed to the repository.
//!
//! The compiled CSS is stored in the repository (instead of being compiled in
//! a build script) so that users of `cot` don't have to compile an SCSS
//! compiler as part of their build.

use std::path::{Path, PathBuf};

/// Pairs of (SCSS source, CSS output) paths, relative to the `cot` crate root.
pub const SCSS_FILES: [(&str, &str); 2] = [
    ("scss/admin/admin.scss", "static/admin/admin.css"),
    ("scss/error.scss", "templates/css/error.css"),
];

/// Returns the path to the `cot` crate root.
///
/// # Panics
///
/// Panics if the workspace path cannot be determined.
#[must_use]
pub fn cot_crate_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("failed to get workspace path")
        .join("cot")
}

/// Compiles the given SCSS file into CSS.
///
/// # Panics
///
/// Panics if the SCSS file fails to compile.
#[must_use]
pub fn compile_scss(scss_path: &Path) -> String {
    let options = grass_compiler::Options::default().style(grass_compiler::OutputStyle::Expanded);
    let mut css = grass_compiler::from_path(scss_path, &options)
        .unwrap_or_else(|e| panic!("failed to compile {}: {e}", scss_path.display()));
    if !css.ends_with('\n') {
        css.push('\n');
    }
    css
}
