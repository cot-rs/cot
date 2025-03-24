//! Middlewares for modifying requests and responses.
//!
//! Middlewares are used to modify requests and responses in a pipeline. They
//! are used to add functionality to the request/response cycle, such as
//! session management, adding security headers, and more.

use std::task::{Context, Poll};

use bytes::Bytes;
use futures_core::future::BoxFuture;
use futures_util::TryFutureExt;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use tower::Service;
use tower_sessions::{MemoryStore, SessionManagerLayer};

use crate::error::ErrorRepr;
use crate::project::MiddlewareContext;
use crate::request::Request;
use crate::response::Response;
use crate::{Body, Error};

/// Middleware that converts a any [`http::Response`] generic type to a
/// [`cot::response::Response`].
///
/// This is useful for converting a response from a middleware that is
/// compatible with the `tower` crate to a response that is compatible with
/// Cot. It's applied automatically by
/// [`RootHandlerBuilder::middleware()`](cot::project::RootHandlerBuilder::middleware())
/// and is not needed to be added manually.
///
/// # Examples
///
/// ```
/// use cot::middleware::LiveReloadMiddleware;
/// use cot::project::{MiddlewareContext, RootHandlerBuilder};
/// use cot::{BoxedHandler, Project, ProjectContext};
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn middlewares(
///         &self,
///         handler: RootHandlerBuilder,
///         context: &MiddlewareContext,
///     ) -> BoxedHandler {
///         handler
///             // IntoCotResponseLayer used internally in middleware()
///             .middleware(LiveReloadMiddleware::from_context(context))
///             .build()
///     }
/// }
/// ```
#[derive(Debug, Copy, Clone)]
pub struct IntoCotResponseLayer;

impl IntoCotResponseLayer {
    /// Create a new [`IntoCotResponseLayer`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::IntoCotResponseLayer;
    ///
    /// let middleware = IntoCotResponseLayer::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for IntoCotResponseLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> tower::Layer<S> for IntoCotResponseLayer {
    type Service = IntoCotResponse<S>;

    fn layer(&self, inner: S) -> Self::Service {
        IntoCotResponse { inner }
    }
}

/// Service struct that converts a any [`http::Response`] generic type to a
/// [`cot::response::Response`].
///
/// Used by [`IntoCotResponseLayer`].
///
/// # Examples
///
/// ```
/// use std::any::TypeId;
///
/// use cot::middleware::{IntoCotResponse, IntoCotResponseLayer};
///
/// assert_eq!(
///     TypeId::of::<<IntoCotResponseLayer as tower::Layer<()>>::Service>(),
///     TypeId::of::<IntoCotResponse::<()>>()
/// );
/// ```
#[derive(Debug, Clone)]
pub struct IntoCotResponse<S> {
    inner: S,
}

impl<S, ResBody, E> Service<Request> for IntoCotResponse<S>
where
    S: Service<Request, Response = http::Response<ResBody>>,
    ResBody: http_body::Body<Data = Bytes, Error = E> + Send + Sync + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = futures_util::future::MapOk<S::Future, fn(http::Response<ResBody>) -> Response>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, request: Request) -> Self::Future {
        self.inner.call(request).map_ok(map_response)
    }
}

fn map_response<ResBody, E>(response: http::response::Response<ResBody>) -> Response
where
    ResBody: http_body::Body<Data = Bytes, Error = E> + Send + Sync + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    response.map(|body| Body::wrapper(BoxBody::new(body.map_err(map_err))))
}

/// Middleware that converts a any error type to a
/// [`cot::Error`].
///
/// This is useful for converting a response from a middleware that is
/// compatible with the `tower` crate to a response that is compatible with
/// Cot. It's applied automatically by
/// [`RootHandlerBuilder::middleware()`](cot::project::RootHandlerBuilder::middleware())
/// and is not needed to be added manually.
///
/// # Examples
///
/// ```
/// use cot::middleware::LiveReloadMiddleware;
/// use cot::project::{MiddlewareContext, RootHandlerBuilder};
/// use cot::{BoxedHandler, Project, ProjectContext};
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn middlewares(
///         &self,
///         handler: RootHandlerBuilder,
///         context: &MiddlewareContext,
///     ) -> BoxedHandler {
///         handler
///             // IntoCotErrorLayer used internally in middleware()
///             .middleware(LiveReloadMiddleware::from_context(context))
///             .build()
///     }
/// }
/// ```
#[derive(Debug, Copy, Clone)]
pub struct IntoCotErrorLayer;

impl IntoCotErrorLayer {
    /// Create a new [`IntoCotErrorLayer`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::IntoCotErrorLayer;
    ///
    /// let middleware = IntoCotErrorLayer::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for IntoCotErrorLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> tower::Layer<S> for IntoCotErrorLayer {
    type Service = IntoCotError<S>;

    fn layer(&self, inner: S) -> Self::Service {
        IntoCotError { inner }
    }
}

/// Service struct that converts a any error type to a [`cot::Error`].
///
/// Used by [`IntoCotErrorLayer`].
///
/// # Examples
///
/// ```
/// use std::any::TypeId;
///
/// use cot::middleware::{IntoCotError, IntoCotErrorLayer};
///
/// assert_eq!(
///     TypeId::of::<<IntoCotErrorLayer as tower::Layer<()>>::Service>(),
///     TypeId::of::<IntoCotError::<()>>()
/// );
/// ```
#[derive(Debug, Clone)]
pub struct IntoCotError<S> {
    inner: S,
}

impl<S> Service<Request> for IntoCotError<S>
where
    S: Service<Request>,
    <S as Service<Request>>::Error: std::error::Error + Send + Sync + 'static,
{
    type Response = S::Response;
    type Error = Error;
    type Future = futures_util::future::MapErr<S::Future, fn(S::Error) -> Error>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(map_err)
    }

    #[inline]
    fn call(&mut self, request: Request) -> Self::Future {
        self.inner.call(request).map_err(map_err)
    }
}

fn map_err<E>(error: E) -> Error
where
    E: std::error::Error + Send + Sync + 'static,
{
    Error::new(ErrorRepr::MiddlewareWrapped {
        source: Box::new(error),
    })
}

/// A middleware that provides session management.
///
/// By default, it uses an in-memory store for session data.
#[derive(Debug, Clone)]
pub struct SessionMiddleware {
    inner: SessionManagerLayer<MemoryStore>,
}

impl SessionMiddleware {
    /// Crates a new instance of [`SessionMiddleware`].
    #[must_use]
    pub fn new() -> Self {
        let store = MemoryStore::default();
        let layer = SessionManagerLayer::new(store);
        Self { inner: layer }
    }
    /// Creates a new instance of [`SessionMiddleware`] from the application
    /// context.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::SessionMiddleware;
    /// use cot::project::{MiddlewareContext, RootHandlerBuilder};
    /// use cot::{BoxedHandler, Project, ProjectContext};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn middlewares(
    ///         &self,
    ///         handler: RootHandlerBuilder,
    ///         context: &MiddlewareContext,
    ///     ) -> BoxedHandler {
    ///         handler
    ///             .middleware(SessionMiddleware::from_context(context))
    ///             .build()
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn from_context(context: &MiddlewareContext) -> Self {
        Self::new().secure(context.config().middlewares.session.secure)
    }

    /// Sets the secure flag for the session middleware.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::SessionMiddleware;
    ///
    /// let middleware = SessionMiddleware::new().secure(false);
    /// ```
    #[must_use]
    pub fn secure(self, secure: bool) -> Self {
        Self {
            inner: self.inner.with_secure(secure),
        }
    }
}

impl Default for SessionMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> tower::Layer<S> for SessionMiddleware {
    type Service = <SessionManagerLayer<MemoryStore> as tower::Layer<
        <SessionWrapperLayer as tower::Layer<S>>::Service,
    >>::Service;

    fn layer(&self, inner: S) -> Self::Service {
        let session_store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(session_store);
        let session_wrapper_layer = SessionWrapperLayer::new();
        let layers = (session_layer, session_wrapper_layer);

        layers.layer(inner)
    }
}

/// A middleware layer that wraps the session object in a
/// [`crate::session::Session`].
///
/// This is only useful inside [`SessionMiddleware`] to expose session object as
/// [`crate::session::Session`] to the request handlers. This shouldn't be
/// useful on its own.
#[derive(Debug, Copy, Clone)]
pub struct SessionWrapperLayer;

impl SessionWrapperLayer {
    /// Create a new [`SessionWrapperLayer`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::SessionWrapperLayer;
    ///
    /// let middleware = SessionWrapperLayer::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SessionWrapperLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> tower::Layer<S> for SessionWrapperLayer {
    type Service = SessionWrapper<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionWrapper { inner }
    }
}

/// Service struct that wraps the session object in a
/// [`crate::session::Session`].
///
/// Used by [`SessionWrapperLayer`].
#[derive(Debug, Clone)]
pub struct SessionWrapper<S> {
    inner: S,
}

impl<ReqBody, ResBody, S> Service<http::Request<ReqBody>> for SessionWrapper<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send,
    ReqBody: Send + 'static,
    ResBody: Default + Send,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<ReqBody>) -> Self::Future {
        let session = req
            .extensions_mut()
            .remove::<tower_sessions::Session>()
            .expect("session extension must be present");
        let session_wrapped = crate::session::Session::new(session);
        req.extensions_mut().insert(session_wrapped);

        self.inner.call(req)
    }
}

/// A middleware that provides authentication functionality.
///
/// This middleware is used to authenticate requests and add the authenticated
/// user to the request extensions. This adds the [`crate::auth::Auth`] object
/// to the request which can be accessed by the request handlers.
///
/// # Examples
///
/// ```
/// use cot::middleware::AuthMiddleware;
/// use cot::project::{MiddlewareContext, RootHandlerBuilder};
/// use cot::{BoxedHandler, Project, ProjectContext};
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn middlewares(
///         &self,
///         handler: RootHandlerBuilder,
///         context: &MiddlewareContext,
///     ) -> BoxedHandler {
///         handler.middleware(AuthMiddleware::new()).build()
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
        // it here and used the version that is ready. This is a common pattern when
        // using `tower::Service`.
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
#[cfg(feature = "live-reload")]
type LiveReloadLayerType = tower::util::Either<
    (
        IntoCotErrorLayer,
        IntoCotResponseLayer,
        tower_livereload::LiveReloadLayer,
    ),
    tower::layer::util::Identity,
>;

/// A middleware providing live reloading functionality.
///
/// This is useful for development, where you want to see the effects of
/// changing your code as quickly as possible. Note that you still need to
/// compile and rerun your project, so it is recommended to combine this
/// middleware with something like [bacon](https://dystroy.org/bacon/).
///
/// This works by serving an additional endpoint that is long-polled in a
/// JavaScript snippet that it injected into the usual response from your
/// service. When the endpoint responds (which happens when the server is
/// started), the website is reloaded. You can see the [`tower_livereload`]
/// crate for more details on the implementation.
///
/// Note that you probably want to have this disabled in the production. You
/// can achieve that by using the [`from_context()`](Self::from_context) method
/// which will read your config to know whether to enable live reloading (by
/// default it will be disabled). Then, you can include the following in your
/// development config to enable it:
///
/// ```toml
/// [middlewares]
/// live_reload.enabled = true
/// ```
///
/// # Examples
///
/// ```
/// use cot::middleware::LiveReloadMiddleware;
/// use cot::project::{MiddlewareContext, RootHandlerBuilder};
/// use cot::{BoxedHandler, Project, ProjectContext};
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn middlewares(
///         &self,
///         handler: RootHandlerBuilder,
///         context: &MiddlewareContext,
///     ) -> BoxedHandler {
///         handler
///             .middleware(LiveReloadMiddleware::from_context(context))
///             .build()
///     }
/// }
/// ```
#[cfg(feature = "live-reload")]
#[derive(Debug, Clone)]
pub struct LiveReloadMiddleware(LiveReloadLayerType);

#[cfg(feature = "live-reload")]
impl LiveReloadMiddleware {
    /// Creates a new instance of [`LiveReloadMiddleware`] that is always
    /// enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::LiveReloadMiddleware;
    /// use cot::project::{MiddlewareContext, RootHandlerBuilder};
    /// use cot::{BoxedHandler, Project, ProjectContext};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn middlewares(
    ///         &self,
    ///         handler: RootHandlerBuilder,
    ///         context: &MiddlewareContext,
    ///     ) -> BoxedHandler {
    ///         // only enable live reloading when compiled in debug mode
    ///         #[cfg(debug_assertions)]
    ///         let handler = handler.middleware(cot::middleware::LiveReloadMiddleware::new());
    ///         handler.build()
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::with_enabled(true)
    }

    /// Creates a new instance of [`LiveReloadMiddleware`] that is enabled if
    /// the corresponding config value is set to `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::LiveReloadMiddleware;
    /// use cot::project::{MiddlewareContext, RootHandlerBuilder};
    /// use cot::{BoxedHandler, Project, ProjectContext};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn middlewares(
    ///         &self,
    ///         handler: RootHandlerBuilder,
    ///         context: &MiddlewareContext,
    ///     ) -> BoxedHandler {
    ///         handler
    ///             .middleware(LiveReloadMiddleware::from_context(context))
    ///             .build()
    ///     }
    /// }
    /// ```
    ///
    /// This will enable live reloading only if the service has the following in
    /// the config file:
    ///
    /// ```toml
    /// [middlewares]
    /// live_reload.enabled = true
    /// ```
    #[must_use]
    pub fn from_context(context: &MiddlewareContext) -> Self {
        Self::with_enabled(context.config().middlewares.live_reload.enabled)
    }

    fn with_enabled(enabled: bool) -> Self {
        let option_layer = enabled.then(|| {
            (
                IntoCotErrorLayer::new(),
                IntoCotResponseLayer::new(),
                tower_livereload::LiveReloadLayer::new(),
            )
        });
        Self(tower::util::option_layer(option_layer))
    }
}

#[cfg(feature = "live-reload")]
impl Default for LiveReloadMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "live-reload")]
impl<S> tower::Layer<S> for LiveReloadMiddleware {
    type Service = <LiveReloadLayerType as tower::Layer<S>>::Service;

    fn layer(&self, inner: S) -> Self::Service {
        self.0.layer(inner)
    }
}

// TODO: add Cot ORM-based session store

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use http::Request;
    use tower::{Layer, ServiceExt};

    use super::*;
    use crate::auth::Auth;
    use crate::session::Session;
    use crate::test::TestRequestBuilder;

    #[tokio::test]
    async fn session_middleware_adds_session() {
        let svc = tower::service_fn(|req: Request<Body>| async move {
            assert!(req.extensions().get::<Session>().is_some());
            Ok::<_, Error>(Response::new(Body::empty()))
        });

        let mut svc = SessionMiddleware::new().layer(svc);

        let request = TestRequestBuilder::get("/").build();

        svc.ready().await.unwrap().call(request).await.unwrap();
    }

    #[tokio::test]
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

    #[tokio::test]
    #[should_panic(
        expected = "Session extension missing. Did you forget to add the SessionMiddleware?"
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

    #[tokio::test]
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
