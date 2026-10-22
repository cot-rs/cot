use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::task::{Context, Poll};

use futures_core::future::BoxFuture;
use tower::util::BoxCloneService;
use tower::{Layer, Service};

use crate::Error;
use crate::request::Request;
use crate::response::{IntoResponse, Response};

/// Creates a middleware from an async function.
///
/// The function receives the [`Request`] and [`Next`], which runs the rest of
/// the middleware stack and the request handler, and returns anything that
/// implements [`IntoResponse`]. State can be captured in a [`Clone`] closure.
///
/// # Examples
///
/// ```
/// use cot::Project;
/// use cot::middleware::{Next, from_fn};
/// use cot::project::{MiddlewareContext, RootHandler, RootHandlerBuilder};
/// use cot::request::Request;
/// use cot::response::{IntoResponse, Redirect, Response};
///
/// async fn redirect_old_paths(request: Request, next: Next) -> cot::Result<Response> {
///     if request.uri().path().starts_with("/old/") {
///         return Redirect::new("/").into_response();
///     }
///
///     let mut response = next.run(request).await?;
///     response
///         .headers_mut()
///         .insert("X-Powered-By", "Cot".parse().unwrap());
///     Ok(response)
/// }
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn middlewares(
///         &self,
///         handler: RootHandlerBuilder,
///         context: &MiddlewareContext,
///     ) -> RootHandler {
///         handler.middleware(from_fn(redirect_old_paths)).build()
///     }
/// }
/// ```
pub fn from_fn<F>(f: F) -> FromFnLayer<F> {
    FromFnLayer { f }
}

/// A [`tower::Layer`] returned by [`from_fn`].
#[derive(Clone)]
pub struct FromFnLayer<F> {
    f: F,
}

impl<F> Debug for FromFnLayer<F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FromFnLayer").finish_non_exhaustive()
    }
}

impl<S, F: Clone> Layer<S> for FromFnLayer<F> {
    type Service = FromFn<F, S>;

    fn layer(&self, inner: S) -> Self::Service {
        FromFn {
            f: self.f.clone(),
            inner,
        }
    }
}

/// A [`tower::Service`] created by [`FromFnLayer`].
#[derive(Clone)]
pub struct FromFn<F, S> {
    f: F,
    inner: S,
}

impl<F, S> Debug for FromFn<F, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FromFn").finish_non_exhaustive()
    }
}

impl<F, Fut, R, S> Service<Request> for FromFn<F, S>
where
    F: Fn(Request, Next) -> Fut,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse,
    S: Service<Request, Response = Response, Error = Error> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        // Only the service instance that was polled is guaranteed to be ready
        let clone = self.inner.clone();
        let inner = std::mem::replace(&mut self.inner, clone);
        let next = Next {
            inner: BoxCloneService::new(inner),
        };

        let future = (self.f)(request, next);
        Box::pin(async move { future.await.into_response() })
    }
}

/// The rest of the middleware stack, including the request handler.
pub struct Next {
    inner: BoxCloneService<Request, Response, Error>,
}

impl Debug for Next {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Next").finish_non_exhaustive()
    }
}

impl Next {
    /// Runs the rest of the middleware stack and the request handler.
    ///
    /// # Errors
    ///
    /// Returns the error of the inner middleware or the request handler.
    pub async fn run(mut self, request: Request) -> crate::Result<Response> {
        self.inner.call(request).await
    }
}

#[cfg(test)]
mod tests {
    use http::header::LOCATION;
    use tower::{Layer, ServiceExt, service_fn};

    use super::*;
    use crate::Body;
    use crate::response::Redirect;
    use crate::test::TestRequestBuilder;

    async fn echo_path(request: Request) -> crate::Result<Response> {
        Ok(Response::new(Body::fixed(request.uri().path().to_owned())))
    }

    async fn response_body(response: Response) -> String {
        let bytes = response.into_body().into_bytes().await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[cot::test]
    async fn modifies_request_and_response() {
        async fn rewrite(mut request: Request, next: Next) -> crate::Result<Response> {
            *request.uri_mut() = "/rewritten/".parse().unwrap();
            let mut response = next.run(request).await?;
            response
                .headers_mut()
                .insert("X-Test", "value".parse().unwrap());
            Ok(response)
        }

        let response = from_fn(rewrite)
            .layer(service_fn(echo_path))
            .oneshot(TestRequestBuilder::get("/test/").build())
            .await
            .unwrap();

        assert_eq!(response.headers()["X-Test"], "value");
        assert_eq!(response_body(response).await, "/rewritten/");
    }

    #[cot::test]
    async fn short_circuits_with_into_response() {
        async fn redirect(_request: Request, _next: Next) -> Redirect {
            Redirect::new("/elsewhere/")
        }

        let response = from_fn(redirect)
            .layer(service_fn(echo_path))
            .oneshot(TestRequestBuilder::get("/test/").build())
            .await
            .unwrap();

        assert!(response.status().is_redirection());
        assert_eq!(response.headers()[LOCATION], "/elsewhere/");
    }

    #[cot::test]
    async fn closure_with_captured_state() {
        let prefix = String::from("/blocked/");
        let layer = from_fn(move |request: Request, next: Next| {
            let blocked = request.uri().path().starts_with(&prefix);
            async move {
                if blocked {
                    Ok(Response::new(Body::fixed("blocked")))
                } else {
                    next.run(request).await
                }
            }
        });

        let service = layer.layer(service_fn(echo_path));
        let blocked = service
            .clone()
            .oneshot(TestRequestBuilder::get("/blocked/page/").build())
            .await
            .unwrap();
        let allowed = service
            .oneshot(TestRequestBuilder::get("/allowed/").build())
            .await
            .unwrap();

        assert_eq!(response_body(blocked).await, "blocked");
        assert_eq!(response_body(allowed).await, "/allowed/");
    }
}
