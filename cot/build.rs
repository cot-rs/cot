use std::path::PathBuf;

#[cfg(all(
    feature = "db",
    not(any(feature = "sqlite", feature = "postgres", feature = "mysql"))
))]
compile_error!("feature \"db\" requires one of: \"sqlite\", \"postgres\", \"mysql\" to be enabled");

fn main() {
    build_css();
}

fn build_css() {
    const SCSS_FILES: [(&str, &str); 2] = [
        ("admin/admin.scss", "static/admin/admin.css"),
        ("error.scss", "templates/css/error.css"),
    ];

    let options = scss_options();

    for (scss_file, css_file) in SCSS_FILES {
        let scss_path = format!("scss/{scss_file}");

        println!("cargo::rerun-if-changed={scss_path}");

        let css = grass::from_path(scss_path, &options).expect("failed to compile SCSS");

        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR should be set");
        let css_path = PathBuf::from(out_dir).join(css_file);
        let css_dir = css_path
            .parent()
            .expect("failed to get CSS parent directory");
        std::fs::create_dir_all(css_dir).expect("failed to create CSS directory");
        std::fs::write(css_path, css).expect("failed to write CSS");
    }
}

fn scss_options() -> grass::Options<'static> {
    grass::Options::default().style(grass::OutputStyle::Compressed)
}
