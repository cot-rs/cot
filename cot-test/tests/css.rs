//! Verifies that the CSS files committed in the `cot` crate are up to date
//! with their SCSS sources.

use cot_test::css::{SCSS_FILES, compile_scss, cot_crate_path};

#[test]
fn css_is_up_to_date() {
    let cot_path = cot_crate_path();

    for (scss_file, css_file) in SCSS_FILES {
        let generated = compile_scss(&cot_path.join(scss_file));

        let css_path = cot_path.join(css_file);
        let committed = std::fs::read_to_string(&css_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", css_path.display()))
            .replace("\r\n", "\n"); // normalize line endings

        assert_eq!(
            generated, committed,
            "cot/{css_file} is out of date with cot/{scss_file}; \
             run `just generate-css` and commit the result"
        );
    }
}
