---
title: OpenAPI
---

One of Cot's powerful features is its ability to automatically generate OpenAPI documentation for your web API. This allows you to create interactive API documentation with minimal configuration, making your APIs more accessible and easier to understand for users and developers.

## Overview

OpenAPI (formerly known as Swagger) is a specification for machine-readable interface files for describing, producing, consuming, and visualizing RESTful web services. Cot provides built-in support for:

1. Automatic OpenAPI specification generation based on your route definitions
2. Integration with Swagger UI for interactive API documentation
3. Type-safe API development with proper schema generation

This chapter will guide you through setting up OpenAPI in your Cot project and show you how to leverage automatic specification generation to create well-documented APIs.

## Prerequisites

To use OpenAPI features in Cot, you need to enable the `openapi` and `swagger-ui` features in your project's `Cargo.toml`:

```toml
[dependencies]
cot = { version = "...", features = ["openapi", "swagger-ui"] }
schemars = "0.9" # Required for JSON Schema generation
```

The `schemars` crate is necessary for creating JSON Schema definitions for your request and response types.

## Setting Up Your API

### Define Your Data Types

First, define your request and response data types with `serde` for serialization and `schemars` for schema generation:

```rust
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Deserialize, JsonSchema)]
struct AddRequest {
    a: i32,
    b: i32,
}

#[derive(Serialize, JsonSchema)]
struct AddResponse {
    result: i32,
}
```

Note the use of `#[derive(JsonSchema)]` which comes from the `schemars` crate. This attribute generates schema information that Cot uses to build the OpenAPI specification.

### Create API Handlers

Next, create your API handlers using Cot's extractors:

```rust
use cot::json::Json;

async fn add(Json(add_request): Json<AddRequest>) -> cot::Result<Json<AddResponse>> {
    let response = AddResponse {
        result: add_request.a + add_request.b,
    };

    Json(response)
}
```

### Use API Method Routers

Instead of using regular method routers, use the OpenAPI-enabled versions that automatically generate API documentation:

```rust
use cot::router::method::openapi::api_post;
use cot::router::{Route, Router};

fn create_router() -> Router {
    Router::with_urls([
        Route::with_api_handler("/add/", api_post(add)),
    ])
}
```

The key differences from standard routes are:

- Using `with_api_handler` instead of `with_handler`
- Using `api_post` instead of `post`

### Register the Swagger UI App

To expose the interactive documentation UI, register the `SwaggerUi` app in your project:

```rust
use cot::openapi::swagger_ui::SwaggerUi;
use cot::static_files::StaticFilesMiddleware;
use cot::{App, AppBuilder, Project};

struct MyProject;

impl Project for MyProject {
    fn middlewares(
        &self,
        handler: RootHandlerBuilder,
        context: &MiddlewareContext,
    ) -> BoxedHandler {
        // StaticFilesMiddleware is required for SwaggerUI to serve its assets
        handler
            .middleware(StaticFilesMiddleware::from_context(context))
            .build()
    }

    fn register_apps(&self, apps: &mut AppBuilder, context: &RegisterAppsContext) {
        // Register the Swagger UI at the /swagger path
        apps.register_with_views(SwaggerUi::new(), "/swagger");

        // Register your API app
        apps.register_with_views(MyApiApp, "");
    }
}
```

Don't forget to include the `StaticFilesMiddleware` as it's required for the Swagger UI to serve its CSS and JavaScript files!

## Complete Example

Here's a complete example of a simple API with OpenAPI documentation:

```rust
use cot::cli::CliMetadata;
use cot::config::ProjectConfig;
use cot::json::Json;
use cot::openapi::swagger_ui::SwaggerUi;
use cot::project::{MiddlewareContext, RegisterAppsContext, RootHandlerBuilder};
use cot::router::method::openapi::api_post;
use cot::router::{Route, Router};
use cot::static_files::StaticFilesMiddleware;
use cot::{App, AppBuilder, BoxedHandler, Project};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, schemars::JsonSchema)]
struct AddRequest {
    a: i32,
    b: i32,
}

#[derive(Serialize, schemars::JsonSchema)]
struct AddResponse {
    result: i32,
}

async fn add(Json(add_request): Json<AddRequest>) -> cot::Result<Json<AddResponse>> {
    let response = AddResponse {
        result: add_request.a + add_request.b,
    };

    Json(response)
}

struct AddApp;

impl App for AddApp {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn router(&self) -> Router {
        Router::with_urls([Route::with_api_handler("/add/", api_post(add))])
    }
}

struct ApiProject;

impl Project for ApiProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        Ok(ProjectConfig::dev_default())
    }

    fn middlewares(
        &self,
        handler: RootHandlerBuilder,
        context: &MiddlewareContext,
    ) -> BoxedHandler {
        handler
            .middleware(StaticFilesMiddleware::from_context(context))
            .build()
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views(SwaggerUi::new(), "/swagger");
        apps.register_with_views(AddApp, "");
    }
}

#[cot::main]
fn main() -> impl Project {
    ApiProject
}
```

After running this example, you can:

1. Navigate to `http://localhost:8000/swagger/` to see the interactive API documentation
2. Test your API directly from the browser using the Swagger UI, or
3. Make requests programmatically:
   ```bash
   curl --header "Content-Type: application/json" \
        --request POST \
        --data '{"a": 123, "b": 456}' \
        'http://localhost:8000/add/'
   ```

## Advanced Features

### Using Path Parameters

Path parameters are automatically detected and included in the OpenAPI specification:

```rust
use cot::request::extractors::Path;

async fn get_user(Path(user_id): Path<i32>) -> cot::Result<Response> {
    // ...
}

// Register the route
Route::with_api_handler("/users/{user_id}", api_get(get_user))
```

### URL Query Parameters

Query parameters are also supported and properly documented:

```rust
use cot::request::extractors::UrlQuery;

#[derive(Deserialize, JsonSchema)]
struct UserQuery {
    active: Option<bool>,
    role: Option<String>,
}

async fn list_users(UrlQuery(query): UrlQuery<UserQuery>) -> cot::Result<Response> {
    // ...
}

// Register the route
Route::with_api_handler("/users", api_get(list_users))
```

### Excluding Routes from OpenAPI Documentation

Sometimes you might want to exclude certain routes from your API documentation. You can do this by using `NoApi`:

```rust
use cot::openapi::NoApi;

// This handler will be in the API docs
Route::with_api_handler("/visible", api_get(visible_handler))

// This handler will work but won't appear in the docs
Route::with_api_handler("/hidden", api_get(NoApi(hidden_handler)))
```

You can also exclude specific parameters from the OpenAPI docs:

```rust
async fn handler(
    Path(id): Path<i32>,                   // Included in OpenAPI docs
    NoApi(context): NoApi<MyContext>,      // Excluded from OpenAPI docs
) -> cot::Result<Response> {
    // ... implementation
}
```

### Multiple HTTP Methods

The `ApiMethodRouter` allows you to define multiple HTTP methods for a single route and include them all in the OpenAPI documentation:

```rust
use cot::router::method::openapi::ApiMethodRouter;

Route::with_api_handler(
    "/items",
    ApiMethodRouter::new()
        .get(list_items)
        .post(create_item)
        .put(update_item)
        .delete(delete_item)
)
```

Each method will be properly documented in the OpenAPI specification.

### Implement your own OpenAPI extractor

In order for your parameter or response type to generate OpenAPI specification, you need to implement the [`ApiOperationPart`](https://docs.rs/cot/0.5/cot/openapi/trait.ApiOperationPart.html) trait. You can study their implementations to understand how to design your own:

* [`Json<T>`](https://docs.rs/cot/0.5/cot/json/struct.Json.html) adds a request or response body to the operation
* [`Path<T>`](https://docs.rs/cot/0.5/cot/request/extractors/struct.Path.html) adds path parameters
* [`UrlQuery<T>`](https://docs.rs/cot/0.5/cot/request/extractors/struct.UrlQuery.html) adds query parameters

The key is to modify the `Operation` object appropriately for your extractor, adding parameters, request bodies, or other OpenAPI elements as needed.

## Conclusion

Cot's OpenAPI integration provides a powerful way to automatically generate comprehensive API documentation while maintaining type safety. By leveraging the schema generation capabilities, you can create well-documented APIs with minimal overhead, making your services more accessible and easier to use.

With just a few additions to your code, you get interactive documentation that stays in sync with your implementation, eliminating the common problem of outdated API docs. This feature is particularly valuable for teams working on APIs that are consumed by external developers or multiple internal teams.
