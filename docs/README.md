# Cot Guide

This directory contains the guide pages for the Cot web framework. The guide is built using Markdown files, which are rendered into HTML by the website engine located in the [cot-site repository](https://github.com/cot-rs/cot-site).

To build the guide with the latest changes, you should go to the [docs-site](../docs-site/) directory and follow the instructions from the [README.md](../docs-site/README.md) file. The changes will be reflected in the website as the "master" version of Cot.

## Testing the code snippets

To ensure the guide remains accurate, all code snippets are automatically tested. You can run these tests using:

```bash
cargo nextest run -p cot-test
# or using the justfile alias
just test-docs
# or its shorter version
just td
```

The test runner identifies snippets by their language and optional configuration (e.g., ` ```rust,has_main `).

### Test Types

- **`rust`**: Snippets are wrapped in an `async` block within a `main` function. Many common symbols from `cot` and `std` are automatically imported. You can use `# ` at the start of a line to include it in the test while hiding it from the rendered guide.
- **`rust,has_main`**: Used for snippets that define their own `main` function. No automatic imports are provided.
- **`toml`**: Snippets are validated by parsing them as a Cot project configuration file.
- **`html.j2`**: Snippets are compiled as Askama templates. The test environment provides dummy files (like `base.html` or `logo.png`) to satisfy common references.
