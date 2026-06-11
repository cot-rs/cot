# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Cot is a Django-inspired, batteries-included Rust web framework built on top of [axum](https://github.com/tokio-rs/axum). It provides an ORM, admin panel, auth system, forms, sessions, caching, email, and OpenAPI—all with a type-safe, macro-driven API. MSRV is **1.88**, edition **2024**.

## Workspace Layout

The repository is a Cargo workspace. The six core crates are:

| Crate | Role |
|---|---|
| `cot` | Main framework library—re-exports everything users interact with |
| `cot-core` | Low-level HTTP primitives (`Request`, `Response`, `Error`, handlers) |
| `cot-macros` | Procedural macros (`#[cot::main]`, `#[derive(Form)]`, `#[derive(Model)]`, etc.) |
| `cot-codegen` | Code generation helpers used by `cot-macros` |
| `cot-cli` | `cot new` scaffolding CLI |
| `cot-test` | Integration/doc-test utilities (unpublished) |

`examples/` contains standalone projects (`hello-world`, `todo-list`, `admin`, etc.) that each have their own `Cargo.toml` but share the workspace lockfile.

All dependency versions are managed in the **workspace root `Cargo.toml`**. Individual crates reference `workspace = true`. The only exception is `examples/`—examples list their own deps so they can be copied standalone.

## Commands

```sh
# Run tests (unit + doctests, no external services needed)
just test
# Equivalent:
cargo nextest run --all-features
cargo test --all-features --doc

# Run integration tests that need Docker (DB, Redis, Selenium)
just test-ignored
# Equivalent:
docker compose up -d --wait
cargo nextest run --all-features --run-ignored only
docker compose down

# Run the full suite
just test-all

# Run a single test by name
cargo nextest run --all-features -E 'test(my_test_name)'

# Lint
just clippy                          # cargo +stable clippy --no-deps --all-targets
just clippy-fix                      # same, with --fix

# Format (enforced by pre-commit)
cargo fmt --all

# Generate docs (requires nightly)
just docs          # no browser
just docs-open     # opens browser

# Coverage (requires cargo-llvm-cov + nightly)
just coverage

# Benchmarks
cargo bench --package cot --features test

# Update snapshot tests after intentional CLI output changes
cargo insta test --review

# Run doc-tests for cot-test specifically
just test-docs     # cargo nextest run -p cot-test
```

Tests that require external services (DB, Redis, Selenium) are marked `#[ignore]` and only run via `just test-ignored` or `--run-ignored only`. The Docker Compose file at the repo root starts all required services. Selenium Grid UI (for debugging E2E tests) is at `http://localhost:7900/?autoconnect=1&resize=scale&password=secret`.

## Architecture

### Project / App model

The two central traits are `Project` and `App` (defined in `cot/src/project.rs`):

- **`App`** is a self-contained feature module. It declares a `name()`, an optional `Router`, database migrations, and initialization hooks. Builtin examples: `cot::auth::db::DatabaseUserApp`, `cot::admin::AdminApp`.
- **`Project`** is the top-level container. Its primary job is `register_apps()`, which wires `App` instances together. It can also set middleware, configure auth backends, and override error pages.
- **`Bootstrapper`** reads config, calls `boot()`, wires middleware layers, and hands off to the async runtime via `#[cot::main]`.

```
#[cot::main] fn main() -> impl Project
    └─ Bootstrapper::new(project).boot().await
        └─ Project::register_apps()  →  one or more App instances
            └─ App::router()  →  Route::with_handler(path, handler_fn)
```

### Routing

`cot/src/router.rs` — Routes map URL paths to async handler functions. Handlers use extractor arguments (pulled from the request by type), similar to axum. The router supports named path parameters and reverse URL resolution (`Router::reverse()`).

### ORM (`#[cfg(feature = "db")]`)

`cot/src/db.rs` + `cot/src/db/` — Built on [sea-query](https://github.com/SeaQL/sea-query). Models are plain Rust structs with `#[derive(Model)]`. The macro generates field metadata, query builders, and migration helpers. Migrations are tracked automatically; the engine diffs model definitions against the database schema. Supports SQLite, PostgreSQL, and MySQL via sqlx.

### Forms

`cot/src/form.rs` — `#[derive(Form)]` generates HTML rendering and validation logic from struct fields. Field types (`CharField`, `IntegerField`, etc.) live in `cot/src/form/fields/`.

### Macros (`cot-macros`)

All public derive and attribute macros live here and are re-exported from `cot`. Key macros:
- `#[cot::main]` / `#[cot::test]` / `#[cot::e2e_test]` — entry-point and test wrappers
- `#[derive(Form)]` — form handling
- `#[derive(Model)]` — ORM model
- `#[derive(AdminModel)]` — admin panel registration
- `#[derive(Query)]` — typed query builders

Compile-error tests for macros use [trybuild](https://github.com/dtolnay/trybuild) and live in `cot-macros/tests/`.

### Admin panel

`cot/src/admin.rs` — Auto-generated management UI. Register a model by implementing `AdminModel` (via derive) and adding it through `AdminApp`. Customizable list display, search, and field ordering.

### Auth

`cot/src/auth.rs` + `cot/src/auth/db/` — Pluggable via `AuthBackend` trait. Default implementation is `DatabaseUserBackend` backed by the ORM. Passwords hashed with argon2 via `password-auth`.

### Templates

Uses [askama](https://github.com/djc/askama) for compile-time checked HTML templates. The `Template` derive (re-exported from askama) is accessed as `cot::Template`. Template files are co-located with source or in a `templates/` directory.

### Feature flags

Major framework features are gated:

| Flag | Enables |
|---|---|
| `db` | ORM, migrations, DB-backed sessions/auth |
| `sqlite` / `postgres` / `mysql` | Database backends |
| `cache` | Memory and Redis caching |
| `email` | Email delivery (SMTP/Sendmail) |
| `openapi` | OpenAPI/Swagger doc generation via aide |
| `test` | `cot::test::TestServerBuilder` and test helpers |
| `json` | JSON request/response support |

### Build script

`cot/build.rs` compiles SCSS (`admin.scss`, `error.scss`) to CSS at build time using `grass_compiler`. Changes to admin or error-page styles require touching the SCSS source files, not the generated CSS.

## Code Conventions

### Lints

Workspace lints (root `Cargo.toml`) are strict:
- `clippy::all` → **deny**
- `clippy::pedantic` → warn
- `missing_docs`, `unreachable_pub`, `unsafe_code` → warn

`clippy.toml` sets `avoid-breaking-exported-api = false`, meaning clippy will warn about breaking API changes.

### Dependencies

Add new dependencies to the **workspace root `Cargo.toml`**, not to individual crate manifests. Pin to the minimum compatible major (or major.minor) version—avoid over-specifying patch versions. This is enforced in CI via minimum-versions checks.

### Testing patterns

- External-dependency tests are annotated `#[ignore]` and run separately.
- CLI output consistency is tested with `cargo-insta` snapshots in `cot-cli/tests/`.
- Browser-based E2E tests use `fantoccini` + Selenium Grid and are also `#[ignore]`.
- `cot::test::TestServerBuilder` (requires `feature = "test"`) provides in-process HTTP testing without a real network.

### Pre-commit hooks

The repo uses [prek](https://prek.j178.dev/) (`.pre-commit-config.yaml`). Hooks run `cargo fmt`, `cargo clippy`, `djlint` (for HTML templates), and YAML/TOML validators. `pre-commit` also works since they share the same config file.
