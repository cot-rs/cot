use std::sync::Arc;
use std::task::{Context, Poll};

use futures_core::future::BoxFuture;
use http::header::LOCATION;
use http::{Method, StatusCode};
use tower::{Layer, Service};

use crate::project::MiddlewareContext;
use crate::request::Request;
use crate::response::{Response, ResponseExt};
use crate::router::Router;
use crate::{Body, Error};

/// Redirects missing GET and HEAD paths to a matching route with a trailing slash.
#[derive(Debug, Clone)]
pub struct TrailingSlashMiddleware {
    router: Arc<Router>,
}

impl TrailingSlashMiddleware {
    /// Creates trailing-slash middleware from the project context.
    #[must_use]
    pub fn from_context(context: &MiddlewareContext) -> Self {
        Self {
            router: Arc::clone(context.router()),
        }
    }
}

impl<S> Layer<S> for TrailingSlashMiddleware {
    type Service = TrailingSlashService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TrailingSlashService {
            inner,
            router: Arc::clone(&self.router),
        }
    }
}

/// Applies trailing-slash redirects before passing requests to a service.
#[derive(Debug, Clone)]
pub struct TrailingSlashService<S> {
    inner: S,
    router: Arc<Router>,
}

impl<S> Service<Request> for TrailingSlashService<S>
where
    S: Service<Request, Response = Response, Error = Error> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        if matches!(*request.method(), Method::GET | Method::HEAD) {
            let path = request.uri().path();
            if !path.ends_with('/') && !self.router.has_route(path) {
                let slash_path = format!("{path}/");
                if self.router.has_route(&slash_path) {
                    let location = match request.uri().query() {
                        Some(query) => format!("{slash_path}?{query}"),
                        None => slash_path,
                    };
                    let response = Response::builder()
                        .status(StatusCode::PERMANENT_REDIRECT)
                        .header(LOCATION, location)
                        .body(Body::empty())
                        .expect("redirect response must be valid");
                    return Box::pin(async move { Ok(response) });
                }
            }
        }

        Box::pin(self.inner.call(request))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use http::header::LOCATION;
    use tower::{Layer, ServiceExt};

    use super::*;
    use crate::router::{Route, RouterService};
    use crate::test::TestRequestBuilder;

    async fn handler(_request: Request) -> crate::Result<Response> {
        Ok(Response::new(Body::fixed("ok")))
    }

    fn service(router: Router) -> TrailingSlashService<RouterService> {
        let router = Arc::new(router);
        TrailingSlashMiddleware {
            router: Arc::clone(&router),
        }
        .layer(RouterService::new(router))
    }

    #[cot::test]
    async fn redirects_get_to_matching_slash_route() {
        let service = service(Router::with_urls([Route::with_handler("/page/", handler)]));
        let response = service
            .oneshot(TestRequestBuilder::get("/page?tab=one").build())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(response.headers()[LOCATION], "/page/?tab=one");
    }

    #[cot::test]
    async fn redirects_head_to_matching_slash_route() {
        let service = service(Router::with_urls([Route::with_handler("/page/", handler)]));
        let response = service
            .oneshot(TestRequestBuilder::with_method("/page", Method::HEAD).build())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(response.headers()[LOCATION], "/page/");
    }

    #[cot::test]
    async fn preserves_existing_route_without_slash() {
        let service = service(Router::with_urls([
            Route::with_handler("/page", handler),
            Route::with_handler("/page/", handler),
        ]));
        let response = service
            .oneshot(TestRequestBuilder::get("/page").build())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[cot::test]
    async fn does_not_redirect_post_requests() {
        let service = service(Router::with_urls([Route::with_handler("/page/", handler)]));
        let error = service
            .oneshot(TestRequestBuilder::post("/page").build())
            .await
            .unwrap_err();

        assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
    }

    #[cot::test]
    async fn leaves_unknown_paths_not_found() {
        let service = service(Router::with_urls([Route::with_handler("/page/", handler)]));
        let error = service
            .oneshot(TestRequestBuilder::get("/missing").build())
            .await
            .unwrap_err();

        assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
    }
}
