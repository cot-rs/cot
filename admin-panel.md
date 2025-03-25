---
title: Admin panel
---

<div class="alert alert-warning" role="alert"><strong>Disclaimer</strong>: Cot is currently missing a lot of features and is <strong>not ready</strong> for production use. This guide is a work in progress and will be updated as Cot matures. That said, you are more than welcome to try it out and provide feedback!</div>

The Cot admin panel provides an automatic interface for managing your models. It allows you to add, edit, delete and view records without writing any custom views or templates. This is perfect for prototyping your application and for managing your data in cases where you don't need a custom interface, as the Cot admin panel is automatically generated based on your models.

## Enabling the Admin Interface

First, add the admin app and the dependencies required to your project in `src/main.rs`:

```rust
use cot::admin::AdminApp;
use cot::auth::db::{DatabaseUser, DatabaseUserApp};
use cot::middleware::{SessionMiddleware, LiveReloadMiddleware};
use cot::project::{WithApps, WithConfig};
use cot::static_files::StaticFilesMiddleware;

struct MyProject;

impl Project for MyProject {
    fn register_apps(&self, apps: &mut AppBuilder, _context: &ProjectContext<WithConfig>) {
        apps.register(DatabaseUserApp::new());  // Needed for admin authentication
        apps.register_with_views(AdminApp::new(), "/admin"); // Register the admin app
        apps.register_with_views(MyApp, "");
    }

    fn middlewares(
        &self,
        handler: cot::project::RootHandlerBuilder,
        app_context: &ProjectContext<WithApps>,
    ) -> BoxedHandler {
        handler
            .middleware(StaticFilesMiddleware::from_app_context(app_context))
            .middleware(SessionMiddleware::new())  // Required for admin login
            .build()
    }

    // ...
}
```

## Admin User Creation

By default, the admin interface uses Cot's authentication system. Therefore, you need to create an admin user if it doesn't exist:

```rust
use std::env;
use cot::auth::db::{DatabaseUser, DatabaseUserCredentials};
use cot::auth::Password;

// In your main.rs:
#[async_trait]
impl App for MyApp {
    async fn init(&self, context: &mut ProjectContext) -> cot::Result<()> {
        // Check if admin user exists
        let admin_username = env::var("ADMIN_USER")
                .unwrap_or_else(|_| "admin".to_string());
        let user = DatabaseUser::get_by_username(context.database(), "admin").await?;
        if user.is_none() {
            let password = env::var("ADMIN_PASSWORD")
                    .unwrap_or_else(|_| "change_me".to_string());
            // Create admin user
            DatabaseUser::create_user(
                context.database(),
                &admin_username,
                &Password::new(&password)
            ).await?;
        }
        Ok(())
    }
}
```

## Registering Models in the Admin

To make your models appear in the admin interface, you need to implement the `AdminModel` trait. The easiest way is to use the `#[derive(AdminModel)]` macro:

```rust
use cot::admin::AdminModel;
use cot::db::{model, Auto};
use cot::form::Form;

#[derive(Debug, Form, AdminModel)]
#[model]
struct BlogPost {
    #[model(primary_key)]
    id: Auto<i32>,
    title: String,
    content: String,
    published: bool,
}
```

Note however that in order to derive the `AdminModel` trait, you need to also derive the `Form` and `Model` traits (the latter is provided by the `#[model]` attribute). In addition to that, your model needs to implement the `Display` trait—for instance, in the case above, we could add it like so:

```rust
impl Display for BlogPost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title)
    }
}
```

After adding the `AdminModel` trait, you can add your model to the admin panel using `DefaultAdminModelManager`. This is as easy as adding the following code to your `App` implementation:

```rust
impl App for MyApp {
    fn admin_model_managers(&self) -> Vec<Box<dyn AdminModelManager>> {
        vec![Box::new(DefaultAdminModelManager::<BlogPost>::new())]
    }

    // ...
}
```

Now your model can be managed through the admin interface at `http://localhost:8000/admin/`!

## Summary

In this chapter, you learned how to enable the Cot admin panel, create an admin user, and register your models in the admin interface. In the next chapter, we'll learn how to handle static assets in Cot.
