//! Regenerates the CSS files in the `cot` crate from their SCSS sources.
//!
//! Run via `just generate-css`.

use cot_test::css::{SCSS_FILES, compile_scss, cot_crate_path};

fn main() {
    let cot_path = cot_crate_path();

    for (scss_file, css_file) in SCSS_FILES {
        let css = compile_scss(&cot_path.join(scss_file));

        let css_path = cot_path.join(css_file);
        let css_dir = css_path
            .parent()
            .expect("failed to get CSS parent directory");
        std::fs::create_dir_all(css_dir).expect("failed to create CSS directory");
        std::fs::write(&css_path, css)
            .unwrap_or_else(|e| panic!("failed to write {}: {e}", css_path.display()));
        println!("Wrote {}", css_path.display());
    }
}
