<div align="center">
<h1><a href="https://cot.rs">Cot Website</a></h1>

[![Docker Build Status](https://github.com/cot-rs/cot/workflows/Docker%20Images/badge.svg)](https://github.com/cot-rs/cot/actions/workflows/docker.yml)
</div>

This contains the sources needed to build the website for the Cot web framework.

## Development

Make sure you have `cargo` installed. You can get it through [rustup](https://rustup.rs/).

Then, the easiest way to run the development server is to run:

```bash
cargo run
```

The website doesn't need any external resources (such as the database), so nothing more is needed.

### Modifying the guide or other Markdown files

Because of the internals of Markdown processing macros work, you will need to use the nightly toolchain if you want to see the changes made to the Markdown files in the without using `cargo clean`.

### Live reloading

To make the development more convenient, you can use [bacon](https://dystroy.org/bacon/) to get live reloading capabilities. After installing it, you can execute:

```bash
bacon serve
```

All the changes you do in Rust source files or the templates should be automatically reflected in the web browser.
