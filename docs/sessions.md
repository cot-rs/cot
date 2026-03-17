---
title: Sessions
---

Sessions allow you to store data about a user across multiple requests. Cot provides a flexible session management system that supports various storage backends like memory, files, databases, or Redis.

## Setting Up Sessions

To enable sessions, you need to add the `SessionMiddleware` to your project's middleware stack.

### Adding the Middleware

In your project's `middlewares` method:

```rust
use cot::middleware::SessionMiddleware;

impl Project for MyProject {
    fn middlewares(
        &self,
        handler: RootHandlerBuilder,
        context: &MiddlewareContext,
    ) -> RootHandler {
        handler
            .middleware(SessionMiddleware::from_context(context))
            .build()
    }
}
```

## Configuring Session Storage

By default, Cot uses a database-backed session store if a database is configured. You can change this in your project configuration:

```rust
use cot::config::{SessionMiddlewareConfig, SessionStoreTypeConfig};

impl Project for MyProject {
    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        Ok(ProjectConfig::builder()
            .middlewares(
                MiddlewareConfig::builder()
                    .session(
                        SessionMiddlewareConfig::builder()
                            .store(SessionStoreTypeConfig::Memory)
                            .build()
                    )
                    .build()
            )
            .build())
    }
}
```

Available session storage backends:

- `Memory`: Store sessions in memory (useful for development).
- `Database`: Store sessions in your database (default).
- `File`: Store sessions in files on disk.
- `Redis`: Store sessions in a Redis database.

## Using Sessions in Views

Use the `Session` extractor to interact with the session object in your views.

```rust
use cot::session::Session;

async fn my_handler(session: Session) -> cot::Result<Html> {
    // Storing a value in the session
    session.insert("user_name", "world".to_string()).await?;

    // Retrieving a value from the session
    let name: Option<String> = session.get("user_name").await?;

    Ok(Html::new(format!("Hello, {}!", name.unwrap_or_else(|| "stranger".to_string()))))
}
```

## Session Lifecycle

- **Creation**: A session is created automatically when you first interact with it.
- **Expiration**: Sessions have a default expiration time that can be configured.
- **Cleanup**: Expired sessions are periodically cleaned up from the storage backend.

## Summary

In this chapter, you learned how to enable and configure sessions in Cot, as well as how to use the `Session` extractor to store and retrieve data across requests. In the next chapter, we'll learn how to handle static assets like CSS and images in Cot.
