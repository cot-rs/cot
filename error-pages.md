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

When the debug mode is disabled, Cot provides default error pages that do not share any information about what happened to the user. To match your service's look and feel, you'll typically want to customize them.

## Custom error handlers

Let's implement a custom error handler in your project:

```rust
use askama::Template;
use cot::html::Html;
use cot::response::{IntoResponse, Response};
use cot::error::handler::{DynErrorPageHandler, RequestError};

async fn error_page_handler(error: RequestError) -> cot::Result<impl IntoResponse> {
    #[derive(Template)]
    #[template(path = "error.html")]
    struct ErrorTemplate {
        error: RequestError,
    }

    let status_code = error.status_code();
    let error_template = ErrorTemplate { error };
    let rendered = error_template.render()?;

    Ok(Html::new(rendered).with_status(status_code))
}

struct MyProject;

impl Project for MyProject {
    fn error_handler(&self) -> DynErrorPageHandler {
        DynErrorPageHandler::new(error_page_handler)
    }
}
```

Create `templates/error.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Error</title>
</head>
<body>
    <h1 class="error-code">{{ error.status_code().as_u16() }}</h1>
    <h2>{{ error.status_code().canonical_reason().unwrap_or("Error") }}</h2>
</body>
</html>
```

Now, try to visit an undefined route or raise an error in your code. You should see the custom error pages you've created!

## Raising errors in views

Cot provides several ways to raise errors in your views:

```rust
use cot::Error;
use cot::error::NotFound;
use cot::request::Request;
use cot::response::Response;

async fn view(request: Request) -> cot::Result<Response> {
    // 404 Not Found
    return Err(NotFound::new())?;

    // 404 with custom message
    return Err(NotFound::with_message(
        "The article you're looking for doesn't exist".to_string()
    ))?;

    // 500 Internal Server Error
    return Err(Error::internal("Something went wrong"));
    // or, by re-raising a custom error:
    return Err(Error::internal(std::io::Error::other("oh no!")));
    // or, by panicking:
    panic!("Something went wrong");
}
```

Note that any messages that you pass to the `Error` structure will only be displayed in debug mode by default. In production, the user will see your custom error pages (which may or may not retrieve the underlying error message, depending on how you implemented them).

## Summary

In this chapter, you learned how to handle errors in Cot applications. You can create custom error pages, raise errors in your views, and overall provide a better user experience when something goes wrong.

Next chapter, we'll explore automatic testing in Cot applications.
