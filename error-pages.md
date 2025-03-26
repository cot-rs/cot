---
title: Error pages
---

Error pages in Cot provide users with helpful information when something goes wrong. Let's learn how to handle errors gracefully and create custom error pages.

## Debug mode error pages

In development (debug mode), Cot provides detailed error pages that include:

* Error message and type
* Stack trace
* Request information
* Configuration details
* Route information

The debug mode is enabled in the default `dev` configuration:

```toml
# config/dev.toml
debug = true
```

Now, when you visit a non-existing page, or if your code raises an error or panics, Cot will display a detailed error page with the information useful to debug the issue. Note that the error pages in debug mode may contain sensitive information, so you should always make sure it is disabled in production!

## Default error pages

When the debug mode is disabled, Cot provides default error pages that do not share any information about what happened to the user. To match your service's look and feel, you'll typically want to customize them. The two types of error pages that can be customized are:

* 404 Not Found
* 500 Internal Server Error

## Custom error handlers

Let's implement custom error handlers in your project:

```rust
use cot::project::{ErrorPageHandler, Project};
use cot::response::{Response, ResponseExt};
use cot::{Body, StatusCode};

struct CustomNotFound;
impl ErrorPageHandler for CustomNotFound {
    fn handle(&self) -> cot::Result<Response> {
        Ok(Response::new_html(
            StatusCode::NOT_FOUND,
            Body::fixed(include_str!("404.html")),
        ))
    }
}

struct CustomServerError;
impl ErrorPageHandler for CustomServerError {
    fn handle(&self) -> cot::Result<Response> {
        Ok(Response::new_html(
            StatusCode::INTERNAL_SERVER_ERROR,
            Body::fixed(include_str!("500.html")),
        ))
    }
}

struct MyProject;

impl Project for MyProject {
    fn not_found_handler(&self) -> Box<dyn ErrorPageHandler> {
        Box::new(CustomNotFound)
    }

    fn server_error_handler(&self) -> Box<dyn ErrorPageHandler> {
        Box::new(CustomServerError)
    }
}
```

Create `404.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>404 Not Found</title>
</head>
<body>
    <h1 class="error-code">404</h1>
    <h2>Page Not Found</h2>
    <p>Oopsies! The page you're looking for doesn't exist.</p>
    <p><a href="/">Return to Homepage</a></p>
</body>
</html>
```

Create `500.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>500 Server Error</title>
</head>
<body>
    <h1 class="error-code">500</h1>
    <h2>Server Error</h2>
    <p>Oopsies! Something went wrong on our end. Please try again later.</p>
    <p><a href="/">Return to Homepage</a></p>
</body>
</html>
```

Now, try to visit an undefined route or raise an error in your code. You should see the custom error pages you've created!

## Raising errors in views

Cot provides several ways to raise errors in your views:

```rust
async fn view(request: Request) -> cot::Result<Response> {
    // 404 Not Found
    return Err(cot::Error::not_found());

    // 404 with custom message
    return Err(cot::Error::not_found_message(
        "The article you're looking for doesn't exist".to_string()
    ));

    // Custom error
    return Err(cot::Error::custom("Something went wrong"));
}
```

Note that any messages that you pass to the `Error` structure will only be displayed in debug mode. In production, the user will see your custom error pages that do not have access to the error message.

## Handling specific errors

You can handle specific errors in your views:

```rust
async fn view_article(RequestDb(db): RequestDb, Path(article_id): Path<i32>) -> cot::Result<Response> {
    // will display a 404 error page if the article ID is below 0
    if article_id < 0 {
        return Error::not_found_message("Invalid article ID".to_string());
    }

    // will display a 404 page if the article is not found in the database
    let article = query!(Article, $id == article_id)
        .get(request.db())
        .await?
        .ok_or_else(|| Error::not_found_message(
            format!("Article {} not found", article_id)
        ))?;

   if article.name.is_empty() {
       // both of these will display a 500 error page:
       return Err(Error::custom("Article name should never be empty!"));
       // or:
       panic!("Article name should never be empty!");
   }

    Ok(Response::new_html(
        StatusCode::OK,
        Body::fixed(render_article(&article)?),
    ))
}
```

## Summary

In this chapter, you learned how to handle errors in Cot applications. You can create custom error pages, raise errors in your views, and be able to handle specific errors.

Next chapter, we'll explore automatic testing in Cot applications.
