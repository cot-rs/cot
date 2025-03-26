---
title: Static files
---

Cot provides a straightforward system for serving static files - resources that don't require server-side processing, such as images, CSS, and JavaScript files.

## Configuration and Usage

### Directory Structure

The Cot CLI generates a `static` directory in your project root, which serves as the designated location for all static files.

### Registering Static Files

To serve static files, you'll need to register them in your application's `static_files()` method within the `CotApp` implementation. Here's a basic example:

```rust
impl CotApp for MyApp {
    fn static_files(&self) -> Vec<(String, Bytes)> {
        static_files!("css/main.css")
    }
}
```

To add more files, simply include them in the `static_files!` macro. For example, after adding a logo to your project:

```rust
impl CotApp for MyApp {
    fn static_files(&self) -> Vec<(String, Bytes)> {
        static_files!(
            "css/main.css",
            "images/logo.png"
        )
    }
}
```

All registered files are automatically served under the `/static` path. For instance, in the example above, you can access the logo at `/static/images/logo.png`.

## Production Deployment

### Collecting Static Files

For production environments, it's recommended to serve static files through specialized services rather than the Cot server for performance and security reasons. Options include:
- Reverse proxy servers (e.g., [nginx](https://nginx.org/), [Caddy](https://caddyserver.com/))
- Content delivery networks (e.g., [Cloudflare](https://www.cloudflare.com/))

To facilitate this deployment strategy, Cot provides a `collect-static` CLI command that consolidates static files from all registered apps (including the Cot Admin app) into a single directory:

```bash
cargo run -- collect-static public/
```

This command aggregates all static files into the specified directory (in this case, `public/`), making them ready for serving through your chosen infrastructure.

### Disabling Static File Serving

If you prefer not to serve static files through the Cot server, you can disable this functionality by removing the `StaticFilesMiddleware` from your project configuration:

```rust
let project = CotProject::builder()
    // ...
    .middleware_with_context(StaticFilesMiddleware::from_app_context)
    .middleware(LiveReloadMiddleware::new())
    .build()
    .await?;
```

Simply remove the `.middleware_with_context(StaticFilesMiddleware ...)` line to disable static file serving.
