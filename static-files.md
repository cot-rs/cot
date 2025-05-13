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
    fn static_files(&self) -> Vec<StaticFile> {
        static_files!("css/main.css")
    }
}
```

To add more files, simply include them in the `static_files!` macro. For example, after adding a logo to your project:

```rust
impl CotApp for MyApp {
    fn static_files(&self) -> Vec<StaticFile> {
        static_files!(
            "css/main.css",
            "images/logo.png",
        )
    }
}
```

You can get the URL for a static file using the `StaticFiles` extractor. For example, to get the URL for the logo:

```rust
use cot::request::extractors::StaticFiles;

async fn get_logo_url(static_files: StaticFiles) -> String {
    static_files.url_for("images/logo.png")
}
```

By default, static files are available at the `/static/` URL prefix. You can configure this in the project config file:

```toml
[static_files]
url = "/assets/"
```

## Caching and versioning

If you used the default project template to create your project, Cot will automatically add a hash as a query parameter in the static files URLs. This allows you to use aggressive caching strategies in production without worrying about cache invalidation. This means that, for instance, for `images/logo.png`, the URL will look like `/static/images/logo.png?v=e3b0c44298fc`, where `e3b0c44298fc` is a hash of the file contents. This way, if the file changes, the URL will change too, and the browser will fetch the new version.

Thanks to the file hashing, you can use aggressive caching strategies in production without worrying about cache invalidation, while ensuring that users won't have to download the same asset twice. This is the default behavior in Cot, but you configure it in the project config file:

```toml
[static_files]
rewrite = "query_param"  # set to "none" to disable hashing
cache_timeout = "1year"  # "Cache-Control" header value
```

Please refer to [humantime crate documentation](https://docs.rs/humantime/latest/humantime/fn.parse_duration.html) on the details about the `cache_timeout` configuration format.

## Production Deployment

### Collecting Static Files

If you want to serve static files through a reverse proxy or CDN, you can use the `collect-static` command to gather all static files into a single directory. This is particularly useful for production environments where you want to serve static files efficiently.

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
