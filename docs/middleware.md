---
title: Middleware
---

Middlewares allow you to intercept and modify requests before they reach your views and responses before they are sent to the client. Cot's middleware system is built on top of [tower](https://docs.rs/tower/latest/tower/), which provides a powerful and standard way to compose services.

## Using Middlewares

Middlewares are added to your project by overriding the `middlewares` method in your `Project` implementation. The `RootHandlerBuilder` provided to this method allows you to stack middlewares.

### Order of Execution

The order in which you add middlewares matters. Middlewares added later "wrap" those added earlier. This means:

1.  **Request Flow**: Middlewares are executed in the **reverse order** of how they were added (the last added is the first to see the request).
2.  **Response Flow**: Middlewares are executed in the **order** they were added (the first added is the last to see the response).

```rust
impl Project for MyProject {
    fn middlewares(
        &self,
        handler: RootHandlerBuilder,
        context: &MiddlewareContext,
    ) -> RootHandler {
        handler
            .middleware(MiddlewareA::new())
            .middleware(MiddlewareB::new())
            .build()
    }
}
```

In the example above, for an incoming request, `MiddlewareB` will be executed first, followed by `MiddlewareA`. For the outgoing response, `MiddlewareA` will be executed first, followed by `MiddlewareB`.

## Built-in Middlewares

Cot comes with several built-in middlewares that provide essential functionality:

- `StaticFilesMiddleware`: Serves static files from your `static/` directory.
- `AuthMiddleware`: Handles user authentication.
- `SessionMiddleware`: Manages user sessions.
- `LiveReloadMiddleware`: Enables live-reloading during development.

## Creating Custom Middlewares

Since Cot uses Tower, you can create custom middlewares by implementing the `Layer` and `Service` traits.

### Example: A Simple Logger Middleware

Here's an example of a simple middleware that logs the incoming request method and path:

```rust
use std::task::{Context, Poll};
use tower::{Layer, Service};
use cot::request::Request;
use cot::response::Response;
use cot::Error;
use futures_util::future::BoxFuture;

pub struct LoggerLayer;

impl<S> Layer<S> for LoggerLayer {
    type Service = LoggerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        LoggerService { inner }
    }
}

pub struct LoggerService<S> {
    inner: S,
}

impl<S> Service<Request> for LoggerService<S>
where
    S: Service<Request, Response = Response, Error = Error> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        println!("Request: {} {}", req.method(), req.uri());

        let mut inner = self.inner.clone();
        Box::pin(async move {
            let res = inner.call(req).await?;
            println!("Response status: {}", res.status());
            Ok(res)
        })
    }
}
```

You can then add it to your project:

```rust
handler.middleware(LoggerLayer).build()
```

## Tower Compatibility

Because Cot's middleware system is based on Tower, you can use many existing Tower layers directly with Cot, provided they are compatible with `http::Request` and `http::Response`.

## Summary

In this chapter, you learned how to use and compose middlewares in Cot, and how to create your own custom middlewares using Tower's standard traits. In the next chapter, we'll explore forms and how Cot handles user input and validation.
