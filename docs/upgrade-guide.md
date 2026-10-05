---
title: Upgrade Guide
---

Each version of Cot introduces new features, improvements, and sometimes breaking changes. This guide will help you understand the changes made in each version and how to adapt your code accordingly.

As a general rule, try to upgrade one minor version at a time. Many breaking changes are introduced by first deprecating a feature in one minor version and then removing it in the next. This gives you time to adapt your code before the feature is removed, while the Rust compiler will notify you about the exact changes you need to make.

Sometimes, though, the changes need to be made in a backwards-incompatible manner. This page will help you understand those changes and how to adapt your code.

## From 0.4 to 0.5

### General

* **MSRV Bump**: The Minimum Supported Rust Version (MSRV) has been bumped to 1.88.
* **Templates**: `cot` now re-exports [`Template`](trait@cot::Template) trait and [`#[derive(Template)]`](macro@cot::Template) macro. You should update your imports from `use askama::Template;` to `use cot::Template;`. This change allows you to remove `askama` from your `Cargo.toml` dependencies.
* **Database**: [`Database`](struct@cot::db::Database) struct now uses `Arc` internally. If you were wrapping [`Database`](struct@cot::db::Database) in `Arc` (e.g. `Arc<Database>`), you should remove the `Arc` wrapper as [`Database`](struct@cot::db::Database) is now cheap to clone.

### Forms

* **Attribute Rename**: The `opt` attribute parameter in `#[form(...)]` macro has been renamed to `opts`.
    ```rust,ignore
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

## From 0.7 to 0.8

### Routing
* **Route Conflicts**: Route registration now rejects ambiguous routes when the router is built. Routes that capture a parameter at the same position must use the same parameter name. Duplicate handler routes also fail during router construction.
    ```rust,ignore
    // Before: these conflicting routes could be registered
    Route::with_handler("/foo/{bar}", handler),
    Route::with_handler("/foo/{baz}", other_handler),

    // After: keep one route for this path pattern
    Route::with_handler("/foo/{bar}", handler),
    ```
* **Handler Takes Precedence Over an Overlapping Router**: If a handler route overlaps with a nested router at the same path, the handler takes precedence for requests matching that path.
    ```rust,ignore
    let nested = Router::with_urls([
        Route::with_handler("/", nested_index),
        Route::with_handler("/details", details),
    ]);

    let router = Router::with_urls([
        Route::with_handler("/foo", foo),
        Route::with_router("/foo", nested),
    ]);

    // GET /foo calls `foo`; the nested router's `/` handler is not reached.
    // GET /foo/details is still served by the nested router.
    ```
* **Trailing Slashes**: Handler routes with and without a trailing slash are distinct routes. Register a handler for each path if both should be available:
    ```rust,ignore
    Route::with_handler("/foo", foo),
    Route::with_handler("/foo/", foo_slash),
    ```
    Nested routers mounted at the same path are merged, including mounts that differ only by a trailing slash. Conflicting routes inside merged routers still fail during router construction.
    ```rust,ignore
    let first = Router::with_urls([Route::with_handler("/a", a)]);
    let second = Router::with_urls([Route::with_handler("/b", b)]);

    let router = Router::with_urls([
        Route::with_router("/users", first),
        Route::with_router("/users/", second),
    ]);

    // Both GET /users/a and GET /users/b are routed successfully.
    ```
