---
title: Routing
---

Routing is the process of mapping incoming requests to specific handler functions (views) based on their URL path. In Cot, you define routes in your app's `router` method using the `Router` and `Route` types.

## Basic Routing

A basic route consists of a path pattern and a handler function. You can also give your route a name, which is useful for URL reversing.

```rust
use cot::router::{Route, Router};

fn router(&self) -> Router {
    Router::with_urls([
        Route::with_handler_and_name("/", index, "home"),
        Route::with_handler_and_name("/about/", about, "about"),
    ])
}
```

## URL Parameters

You can define dynamic routes with parameters enclosed in curly braces `{}`. These parameters can be extracted in your view using the `Path` extractor.

```rust
Route::with_handler_and_name("/user/{id}/", user_profile, "user_profile")
```

The corresponding view:

```rust
use cot::request::extractors::Path;

async fn user_profile(Path(id): Path<i32>) -> cot::Result<Html> {
    // ...
}
```

## URL Reversing

URL reversing is the process of generating a URL based on the name of a route and its parameters. This is preferred over hardcoding URLs in your templates and views, as it makes your application more maintainable.

### Using the `reverse!` Macro

In your views, you can use the `reverse!` macro. This requires the `Urls` extractor:

```rust
use cot::reverse;
use cot::request::extractors::Urls;

async fn my_view(urls: Urls) -> cot::Result<Response> {
    let home_url = reverse!(urls, "home")?;
    let user_url = reverse!(urls, "user_profile", id = 42)?;

    // ...
}
```

### In Templates

In HTML templates, you can use the `url` function provided by the `Urls` object:

```html
<a href="{{ urls.url('home') }}">Home</a>
<a href="{{ urls.url('user_profile', id=user.id) }}">Profile</a>
```

## App Namespacing

When you register an app in your project, you can specify a path prefix for its routes. This prefix will be automatically added to all routes in that app.

```rust
impl Project for MyProject {
    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views(BlogApp::new(), "/blog");
    }
}
```

In this case, a route named `post_detail` in `BlogApp` will be available under `/blog/...`. When reversing, you don't need to worry about this prefix; Cot handles it for you.

## Multiple Routers

For larger applications, you can split your routes into multiple routers and include them in each other.

```rust
fn blog_router() -> Router {
    Router::with_urls([
        Route::with_handler_and_name("/{slug}/", post_detail, "post_detail"),
    ])
}

fn main_router() -> Router {
    Router::with_urls([
        Route::with_handler_and_name("/", index, "index"),
    ]).include("/blog", blog_router())
}
```

## Summary

In this chapter, you learned how to define basic and dynamic routes in Cot, how to use URL reversing to generate URLs dynamically, and how to organize your routes using prefixes and included routers. In the next chapter, we'll dive deeper into templates, which will allow us to create more sophisticated HTML pages.
