---
title: Authentication
---

Cot includes a powerful and flexible authentication system that handles user sessions, password hashing, and user management. It's designed to be secure by default while remaining easy to use and extend.

## Key Concepts

The authentication system is built around three main components:

- **User**: A trait that represents an identity in your system.
- **Backend**: A trait responsible for retrieving users based on credentials or an ID.
- **Middleware**: Handles the lifecycle of an authenticated session for each request.

## Setting Up Authentication

To use authentication in your project, you need to configure it in several places.

### 1. Register the Database User App

If you want to use the built-in database-backed user system, you need to register the `DatabaseUserApp` in your project's `register_apps` method:

```rust
use cot::auth::db::DatabaseUserApp;

impl Project for MyProject {
    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register(DatabaseUserApp::new());
        // ... your other apps
    }
}
```

### 2. Configure the Auth Backend

In your project configuration, you need to specify which authentication backend to use. The most common choice is the database backend:

```rust
use cot::config::{AuthBackendConfig, ProjectConfig};

impl Project for MyProject {
    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        Ok(ProjectConfig::builder()
            .auth_backend(AuthBackendConfig::Database)
            // ... other config
            .build())
    }
}
```

### 3. Add the Middlewares

The authentication system requires both `SessionMiddleware` and `AuthMiddleware` to be present in your middleware stack. The `SessionMiddleware` **must** wrap the `AuthMiddleware`, which means it should be added *after* it in the `middlewares` method:

```rust
use cot::middleware::{AuthMiddleware, SessionMiddleware};

impl Project for MyProject {
    fn middlewares(
        &self,
        handler: RootHandlerBuilder,
        context: &MiddlewareContext,
    ) -> RootHandler {
        handler
            .middleware(AuthMiddleware::new())
            .middleware(SessionMiddleware::from_context(context))
            .build()
    }
}
```

## Using Authentication in Views

Once set up, you can use the `Auth` extractor to interact with the authentication system in your views.

### Checking if a User is Authenticated

The `Auth` extractor provides a `user()` method that returns a `User` object. You can use it to check if the user is logged in:

```rust
use cot::auth::Auth;

async fn profile(auth: Auth) -> cot::Result<Html> {
    let user = auth.user();

    if user.is_authenticated() {
        Ok(Html::new(format!("Welcome back, {}!", user.username())))
    } else {
        Ok(Html::new("Please log in."))
    }
}
```

### Logging In and Out

To log a user in, you can use the `login` method on the `Auth` object. This requires a user object that you've retrieved from your backend.

```rust
use cot::auth::Auth;
use cot::auth::db::DatabaseUser;

async fn login_view(auth: Auth, db: Database) -> cot::Result<Response> {
    // In a real app, you would verify credentials here
    let user = DatabaseUser::get_by_username(&db, "admin").await?.unwrap();

    auth.login(&user).await?;
    Ok(Redirect::to("/").into_response())
}
```

Similarly, use `logout` to end the user's session:

```rust
async fn logout_view(auth: Auth) -> cot::Result<Response> {
    auth.logout().await?;
    Ok(Redirect::to("/").into_response())
}
```

## Password Hashing

Cot provides built-in utilities for secure password hashing using industry-standard algorithms.

```rust
use cot::auth::PasswordHash;

// Hashing a password
let password = "my_secure_password";
let hashed = PasswordHash::new(password)?;

// Verifying a password
if hashed.verify(password).is_ok() {
    println!("Password is correct!");
}
```

When using the `DatabaseUser`, this hashing is handled for you when you use `DatabaseUser::create_user`.

## Custom User Models

While `DatabaseUser` is provided for convenience, you can implement your own user model by implementing the `User` trait. This is useful if you need to store additional data about your users or use a different database structure.

```rust
use cot::auth::User;

#[model]
pub struct MyCustomUser {
    #[model(primary_key)]
    id: Auto<i64>,
    username: String,
    // ... custom fields
}

impl User for MyCustomUser {
    fn id(&self) -> Option<UserId> {
        Some(UserId::new(self.id))
    }

    fn username(&self) -> &str {
        &self.username
    }

    // ... implement other methods
}
```

## Summary

In this chapter, you learned how to set up Cot's authentication system, how to use it in your views to manage user sessions, and how to work with password hashes. In the next chapter, we'll dive into sessions and how they allow you to store persistent data about your users.
