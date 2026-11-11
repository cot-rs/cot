//! The authentication middleware.

use std::task::{Context, Poll};

use futures_core::future::BoxFuture;
use tower::Service;

use crate::Error;
use crate::request::Request;
use crate::response::Response;

/// A middleware that provides authentication functionality.
///
/// This middleware is used to authenticate requests and add the authenticated
/// user to the request extensions. This adds the [`crate::auth::Auth`] object
/// to the request which can be accessed by the request handlers.
///
/// This middleware requires [`SessionMiddleware`](super::SessionMiddleware).
/// Since each
/// [`RootHandlerBuilder::middleware`](crate::project::RootHandlerBuilder::middleware) call wraps the existing handler, register
/// `SessionMiddleware` after `AuthMiddleware` so session handling runs first.
///
/// # Examples
///
/// ```
/// use cot::middleware::{AuthMiddleware, SessionMiddleware};
/// use cot::project::{MiddlewareContext, RootHandler, RootHandlerBuilder};
/// use cot::{Project, ProjectContext};
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn middlewares(
///         &self,
///         handler: RootHandlerBuilder,
///         context: &MiddlewareContext,
///     ) -> RootHandler {
///         handler
///             .middleware(AuthMiddleware::new())
///             .middleware(SessionMiddleware::from_context(context))
///             .build()
///     }
/// }
/// ```
#[derive(Debug, Copy, Clone)]
pub struct AuthMiddleware;

impl AuthMiddleware {
    /// Create a new [`AuthMiddleware`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::AuthMiddleware;
    ///
    /// let middleware = AuthMiddleware::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for AuthMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> tower::Layer<S> for AuthMiddleware {
    type Service = AuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthService::new(inner)
    }
}

/// Service that adds [`crate::auth::Auth`] to the request.
///
/// Used by [`AuthMiddleware`].
#[derive(Debug, Clone)]
pub struct AuthService<S> {
    inner: S,
}

impl<S> AuthService<S> {
    fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> Service<Request> for AuthService<S>
where
    S: Service<Request, Response = Response, Error = Error> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = S::Response;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request) -> Self::Future {
        // The inner service may panic until ready, so it's important to clone
        // it here and used the version that is ready. This is a common pattern
        // when using `tower::Service`.
        //
        // https://docs.rs/tower/latest/tower/trait.Service.html#be-careful-when-cloning-inner-services
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            let auth = crate::auth::Auth::from_request(&mut req).await?;
            req.extensions_mut().insert(auth);

            inner.call(req).await
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use http::Request;
    use tower::{Layer, Service, ServiceExt};

    use super::*;
    use crate::Body;
    use crate::auth::Auth;
    use crate::test::TestRequestBuilder;

    #[cot::test]
    async fn auth_middleware_adds_auth() {
        let svc = tower::service_fn(|req: Request<Body>| async move {
            let auth = req
                .extensions()
                .get::<Auth>()
                .expect("Auth should be present");

            assert!(!auth.user().is_authenticated());

            Ok::<_, Error>(Response::new(Body::empty()))
        });

        let mut svc = AuthMiddleware::new().layer(svc);

        let request = TestRequestBuilder::get("/").with_session().build();

        svc.ready().await.unwrap().call(request).await.unwrap();
    }

    #[cot::test]
    #[should_panic(
        expected = "Session extension missing. Add SessionMiddleware, and if using AuthMiddleware, register SessionMiddleware after AuthMiddleware."
    )]
    async fn auth_middleware_requires_session() {
        let svc = tower::service_fn(|_req: Request<Body>| async move {
            Ok::<_, Error>(Response::new(Body::empty()))
        });

        let mut svc = AuthMiddleware::new().layer(svc);

        let request = TestRequestBuilder::get("/").build();

        // Should fail because Auth middleware requires session
        let _result = svc.ready().await.unwrap().call(request).await;
    }

    #[cot::test]
    async fn auth_service_cloning() {
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let svc = tower::service_fn(move |req: Request<Body>| {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                assert!(req.extensions().get::<Auth>().is_some());

                Ok::<_, Error>(Response::new(Body::empty()))
            }
        });

        let mut svc = AuthMiddleware::new().layer(svc);
        let svc = svc.ready().await.unwrap();

        // Send multiple requests to test service cloning
        let request1 = TestRequestBuilder::get("/").with_session().build();
        let request2 = TestRequestBuilder::get("/").with_session().build();

        // Process requests concurrently
        let (res1, res2) = tokio::join!(svc.clone().call(request1), svc.call(request2));

        assert!(res1.is_ok());
        assert!(res2.is_ok());

        // Counter should have been incremented twice
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
