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
