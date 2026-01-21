---
title: Upgrade Guide
---

Each version of Cot introduces new features, improvements, and sometimes breaking changes. This guide will help you understand the changes made in each version and how to adapt your code accordingly.

As a general rule, try to upgrade one minor version at a time. Many breaking changes are introduced by first deprecating a feature in one minor version and then removing it in the next. This gives you time to adapt your code before the feature is removed, while the Rust compiler will notify you about the exact changes you need to make.

Sometimes, though, the changes need to be made in a backwards-incompatible manner. This page will help you understand those changes and how to adapt your code.

## From 0.4 to 0.5

### General

* **MSRV Bump**: The Minimum Supported Rust Version (MSRV) has been bumped to 1.88.
* **Templates**: `cot` now re-exports `Template` trait and `#[derive(Template)]` macro. You should update your imports from `use askama::Template;` to `use cot::Template;`. This change allows you to remove `askama` from your `Cargo.toml` dependencies.
* **Database**: `Database` struct now uses `Arc` internally. If you were wrapping `Database` in `Arc` (e.g. `Arc<Database>`), you should remove the `Arc` wrapper as `Database` is now cheap to clone.

### Forms

* **Attribute Rename**: The `opt` attribute parameter in `#[form(...)]` macro has been renamed to `opts`.
    ```rust
    // Before
    #[form(opt(max_length = 100))]

    // After
    #[form(opts(max_length = 100))]
    ```

### Configuration

* **Cache Support**: Cot now includes a built-in caching system. This brings a new `[cache]` section in the configuration. If you have any existing configuration that conflicts with this, you might need to adjust it.
* **Email Support**: Similar to caching, email support has been added with a new `[email]` configuration section.

## From 0.3 to 0.4

### General

* `FromRequestParts` is called `FromRequestHead` now. Similarly, `FromRequestParts::from_request_parts` is now `FromRequestHead::from_request_head`.
* `axum::request::Parts` is now re-exported as `cot::request::RequestHead`.
* `axum::response::Parts` is now re-exported as `cot::response::ResponseHead`.

### Error handling

* "Not Found" handler support has been removed. Instead, there is a single project-global error handler that handles both "Not Found", "Internal Server Error", and other errors that may occur during request processing.
* The error handler is now almost a regular request handler (meaning you don't have to implement the `ErrorHandler` trait manually) and can access most of the request data, such as request path, method, headers, but also static files, root router URLs, and more.
  - The main difference between a regular request handler and an error handler is that the error handler may receive an additional argument of type `RequestError`, which contains information about the error that occurred during request processing.
  - On the other hand, it can **not** receive the request body, as it might have been consumed already.
* `Project::server_error_handler` method is now called `error_handler` and returns a `DynErrorPageHandler`.

### Dependencies

* `schemars` dependency has been updated to `0.9`. If you have any custom code to generate OpenAPI specs, (usually by implementing `AsApiOperation`, `ApiOperationPart`, or `AsApiOperation` traits inside `cot::openapi`) you may need to update it accordingly. If you're only using Cot's built-in OpenAPI support, you don't need to do anything except updating your `Cargo.toml` file.
