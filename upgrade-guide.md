---
title: Upgrade Guide
---

Each version of Cot introduces new features, improvements, and sometimes breaking changes. This guide will help you understand the changes made in each version and how to adapt your code accordingly.

As a general rule, try to upgrade one minor version at a time. Many breaking changes are introduced by first deprecating a feature in one minor version and then removing it in the next. This gives you time to adapt your code before the feature is removed, while the Rust compiler will notice you about the exact changes you need to make.

Sometimes, though, the changes need to be made in a backwards-incompatible manner. This page will help you understand those changes and how to adapt your code.

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
