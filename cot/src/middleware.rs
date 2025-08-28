//! Middlewares for modifying requests and responses.
//!
//! Middlewares are used to modify requests and responses in a pipeline. They
//! are used to add functionality to the request/response cycle, such as
//! session management, adding security headers, and more.

use std::borrow::Cow;
use std::fmt::Debug;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_core::future::BoxFuture;
use futures_util::TryFutureExt;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use tower::Service;
use tower_sessions::service::PlaintextCookie;
use tower_sessions::{SessionManagerLayer, SessionStore};

#[cfg(feature = "cache")]
use crate::config::CacheType;
use crate::config::{Expiry, SameSite, SessionStoreTypeConfig};
use crate::project::MiddlewareContext;
use crate::request::Request;
use crate::response::Response;
use crate::session::store::SessionStoreWrapper;
#[cfg(all(feature = "db", feature = "json"))]
use crate::session::store::db::DbStore;
#[cfg(feature = "json")]
use crate::session::store::file::FileStore;
use crate::session::store::memory::MemoryStore;
#[cfg(feature = "redis")]
use crate::session::store::redis::RedisStore;
use crate::{Body, Error};

#[cfg(feature = "live-reload")]
mod live_reload;

#[cfg(feature = "live-reload")]
pub use live_reload::LiveReloadMiddleware;

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
/// use cot::Project;
/// use cot::middleware::LiveReloadMiddleware;
/// use cot::project::{MiddlewareContext, RootHandler, RootHandlerBuilder};
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn middlewares(
///         &self,
///         handler: RootHandlerBuilder,
///         context: &MiddlewareContext,
///     ) -> RootHandler {
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

/// Service struct that converts any [`http::Response`] generic type to
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

/// Middleware that converts any error type to [`cot::Error`].
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
/// use cot::Project;
/// use cot::middleware::LiveReloadMiddleware;
/// use cot::project::{MiddlewareContext, RootHandler, RootHandlerBuilder};
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn middlewares(
///         &self,
///         handler: RootHandlerBuilder,
///         context: &MiddlewareContext,
///     ) -> RootHandler {
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
    #[expect(trivial_casts)]
    let boxed = Box::new(error) as Box<dyn std::error::Error + Send + Sync>;
    boxed.downcast::<Error>().map_or_else(Error::wrap, |e| *e)
}

type DynamicSessionStore = SessionManagerLayer<SessionStoreWrapper, PlaintextCookie>;

/// A middleware that provides session management.
///
/// By default, it uses an in-memory store for session data.
#[derive(Debug, Clone)]
pub struct SessionMiddleware {
    inner: DynamicSessionStore,
}

impl SessionMiddleware {
    /// Crates a new instance of [`SessionMiddleware`].
    #[must_use]
    pub fn new<S: SessionStore + Send + Sync + 'static>(store: S) -> Self {
        let layer = SessionManagerLayer::new(SessionStoreWrapper::new(Arc::new(store)));
        SessionMiddleware { inner: layer }
    }

    /// Creates a new instance of [`SessionMiddleware`] from the application
    /// context.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Project;
    /// use cot::middleware::SessionMiddleware;
    /// use cot::project::{MiddlewareContext, RootHandler, RootHandlerBuilder};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn middlewares(
    ///         &self,
    ///         handler: RootHandlerBuilder,
    ///         context: &MiddlewareContext,
    ///     ) -> RootHandler {
    ///         handler
    ///             .middleware(SessionMiddleware::from_context(context))
    ///             .build()
    ///     }
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// Will panic if the session store type is not supported.
    #[must_use]
    pub fn from_context(context: &MiddlewareContext) -> Self {
        let session_cfg = &context.config().middlewares.session;
        let store_type = session_cfg.store.store_type.clone();
        let boxed_store = Self::config_to_session_store(store_type, context);
        let arc_store = Arc::from(boxed_store);
        let layer = SessionManagerLayer::new(SessionStoreWrapper::new(arc_store));
        let mut middleware = SessionMiddleware { inner: layer }
            .secure(session_cfg.secure)
            .path(session_cfg.path.clone())
            .name(session_cfg.name.clone())
            .http_only(session_cfg.http_only)
            .always_save(session_cfg.always_save)
            .same_site(session_cfg.same_site)
            .expiry(session_cfg.expiry);

        if let Some(domain) = session_cfg.domain.as_ref() {
            middleware = middleware.domain(domain.clone());
        }
        middleware
    }

    /// Sets the secure flag for the session middleware.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::SessionMiddleware;
    /// use cot::session::store::memory::MemoryStore;
    ///
    /// let store = MemoryStore::new();
    /// let middleware = SessionMiddleware::new(store).secure(false);
    /// ```
    #[must_use]
    pub fn secure(self, secure: bool) -> Self {
        let layer = self.inner.with_secure(secure);
        SessionMiddleware { inner: layer }
    }

    /// Enables or disables the `HttpOnly` flag on the session cookie.
    ///
    /// When `true`, the cookie is inaccessible to JavaScript.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::SessionMiddleware;
    /// use cot::session::store::memory::MemoryStore;
    ///
    /// let store = MemoryStore::new();
    /// let middleware = SessionMiddleware::new(store).http_only(false);
    /// ```
    #[must_use]
    pub fn http_only(self, http_only: bool) -> Self {
        Self {
            inner: self.inner.with_http_only(http_only),
        }
    }

    /// Sets the `Domain` attribute for the session cookie.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::SessionMiddleware;
    /// use cot::session::store::memory::MemoryStore;
    ///
    /// let store = MemoryStore::new();
    /// let middleware = SessionMiddleware::new(store).domain("example.com");
    /// ```
    #[must_use]
    pub fn domain<D: Into<Cow<'static, str>>>(self, domain: D) -> Self {
        Self {
            inner: self.inner.with_domain(domain),
        }
    }

    /// Sets the `SameSite` attribute for the session cookie.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::SameSite;
    /// use cot::middleware::SessionMiddleware;
    /// use cot::session::store::memory::MemoryStore;
    ///
    /// let store = MemoryStore::new();
    /// let middleware = SessionMiddleware::new(store).same_site(SameSite::Lax);
    /// ```
    #[must_use]
    pub fn same_site(self, same_site: SameSite) -> Self {
        Self {
            inner: self.inner.with_same_site(same_site.into()),
        }
    }

    /// Sets the cookie **name** (default `"id"`).
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::SessionMiddleware;
    /// use cot::session::store::memory::MemoryStore;
    ///
    /// let store = MemoryStore::new();
    /// let middleware = SessionMiddleware::new(store).name("session_id");
    /// ```
    #[must_use]
    pub fn name<N: Into<Cow<'static, str>>>(self, name: N) -> Self {
        Self {
            inner: self.inner.with_name(name.into()),
        }
    }

    /// Sets the cookie **path** (default `"/"`).
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::SessionMiddleware;
    /// use cot::session::store::memory::MemoryStore;
    ///
    /// let store = MemoryStore::new();
    /// let middleware = SessionMiddleware::new(store).path("/api");
    /// ```
    #[must_use]
    pub fn path<P: Into<Cow<'static, str>>>(self, path: P) -> Self {
        Self {
            inner: self.inner.with_path(path.into()),
        }
    }

    /// When `true`, always writes back the session even if unmodified.
    ///
    /// Useful for resetting expiry on every request.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::SessionMiddleware;
    /// use cot::session::store::memory::MemoryStore;
    ///
    /// let store = MemoryStore::new();
    /// let middleware = SessionMiddleware::new(store).always_save(true);
    /// ```
    #[must_use]
    pub fn always_save(self, always_save: bool) -> Self {
        Self {
            inner: self.inner.with_always_save(always_save),
        }
    }

    /// Sets the expiry behavior for the session cookie.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use cot::config::Expiry;
    /// use cot::middleware::SessionMiddleware;
    /// use cot::session::store::memory::MemoryStore;
    ///
    /// let store = MemoryStore::new();
    /// let middleware =
    ///     SessionMiddleware::new(store).expiry(Expiry::OnInactivity(Duration::from_secs(3600)));
    /// ```
    #[must_use]
    pub fn expiry(self, expiry: Expiry) -> Self {
        Self {
            inner: self.inner.with_expiry(expiry.into()),
        }
    }

    /// Convert a [`SessionStoreTypeConfig`] variant into a valid
    /// [`SessionStore`]
    fn config_to_session_store(
        config: SessionStoreTypeConfig,
        context: &MiddlewareContext,
    ) -> Box<dyn SessionStore + Send + Sync> {
        match config {
            SessionStoreTypeConfig::Memory => Box::new(MemoryStore::new()),
            #[cfg(feature = "json")]
            SessionStoreTypeConfig::File { path } => Box::new(
                FileStore::new(path)
                    .unwrap_or_else(|err| panic!("could not create File store: {err}")),
            ),
            #[cfg(feature = "cache")]
            SessionStoreTypeConfig::Cache { ref uri } => {
                let cache_type = CacheType::try_from(uri.clone())
                    .unwrap_or_else(|e| panic!("could not convert cache URI `{uri}`: {e}"));
                match cache_type {
                    #[cfg(feature = "redis")]
                    CacheType::Redis => {
                        Box::new(RedisStore::new(uri).unwrap_or_else(|e| {
                            panic!("could not connect to Redis at `{uri}`: {e}")
                        }))
                    }
                }
            }
            #[cfg(all(feature = "db", feature = "json"))]
            SessionStoreTypeConfig::Database => Box::new(DbStore::new(context.database().clone())),
        }
    }
}

impl Default for SessionMiddleware {
    fn default() -> Self {
        let memory_store = MemoryStore::default();
        Self::new(memory_store)
    }
}

impl<S> tower::Layer<S> for SessionMiddleware {
    type Service = <DynamicSessionStore as tower::Layer<
        <SessionWrapperLayer as tower::Layer<S>>::Service,
    >>::Service;

    fn layer(&self, inner: S) -> Self::Service {
        let session_wrapper_layer = SessionWrapperLayer::new();
        let layers = (&self.inner, session_wrapper_layer);

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

// TODO: add Cot ORM-based session store

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::PathBuf;
    use std::sync::Arc;

    use http::Request;
    use tower::{Layer, Service, ServiceExt};

    use super::*;
    use crate::auth::Auth;
    use crate::config::{
        CacheUrl, DatabaseConfig, MiddlewareConfig, ProjectConfig, SessionMiddlewareConfig,
        SessionStoreConfig, SessionStoreTypeConfig,
    };
    use crate::middleware::SessionMiddleware;
    use crate::project::{RegisterAppsContext, WithDatabase};
    use crate::response::Response;
    use crate::session::Session;
    use crate::test::TestRequestBuilder;
    use crate::{AppBuilder, Body, Bootstrapper, Error, Project, ProjectContext};

    #[cot::test]
    async fn session_middleware_adds_session() {
        let svc = tower::service_fn(|req: Request<Body>| async move {
            assert!(req.extensions().get::<Session>().is_some());
            Ok::<_, Error>(Response::new(Body::empty()))
        });
        let store = MemoryStore::default();
        let mut svc = SessionMiddleware::new(store).layer(svc);
        let request = TestRequestBuilder::get("/").build();
        svc.ready().await.unwrap().call(request).await.unwrap();
    }

    #[cot::test]
    async fn session_middleware_adds_cookie() {
        let svc = tower::service_fn(|req: Request<Body>| async move {
            let session = req.extensions().get::<Session>().unwrap();
            session.insert("test", "test").await.unwrap();

            Ok::<_, Error>(Response::new(Body::empty()))
        });
        let store = MemoryStore::default();
        let mut svc = SessionMiddleware::new(store)
            .domain("example.com")
            .layer(svc);

        let request = TestRequestBuilder::get("/").build();

        let response = svc.ready().await.unwrap().call(request).await.unwrap();
        assert!(response.headers().contains_key("set-cookie"));
        let cookie_value = response
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap();

        assert!(cookie_value.contains("id="));
        assert!(cookie_value.contains("HttpOnly;"));
        assert!(cookie_value.contains("SameSite=Strict;"));
        assert!(cookie_value.contains("Secure;"));
        assert!(cookie_value.contains("Path=/"));
        assert!(cookie_value.contains("Domain=example.com"));
    }

    #[cot::test]
    async fn session_middleware_adds_cookie_not_secure() {
        let svc = tower::service_fn(|req: Request<Body>| async move {
            let session = req.extensions().get::<Session>().unwrap();
            session.insert("test", "test").await.unwrap();

            Ok::<_, Error>(Response::new(Body::empty()))
        });

        let store = MemoryStore::default();
        let mut svc = SessionMiddleware::new(store).secure(false).layer(svc);

        let request = TestRequestBuilder::get("/").build();

        let response = svc.ready().await.unwrap().call(request).await.unwrap();
        let cookie_value = response
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(!cookie_value.contains("Secure;"));
    }

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

    async fn create_svc_and_call_with_req(context: &ProjectContext<WithDatabase>) {
        let store = SessionMiddleware::from_context(context);
        let svc = tower::service_fn(|req: Request<Body>| async move {
            assert!(req.extensions().get::<Session>().is_some());
            Ok::<_, Error>(Response::new(Body::empty()))
        });
        let mut svc = store.layer(svc);
        let request = TestRequestBuilder::get("/").build();
        svc.ready().await.unwrap().call(request).await.unwrap();
    }

    fn create_project_config(store: SessionStoreTypeConfig) -> ProjectConfig {
        let mut project = ProjectConfig::builder();
        let project = match store {
            SessionStoreTypeConfig::Database => project.database(
                DatabaseConfig::builder()
                    .url("sqlite::memory:".to_string())
                    .build(),
            ),
            _ => &mut project,
        };

        project
            .middlewares(
                MiddlewareConfig::builder()
                    .session(
                        SessionMiddlewareConfig::builder()
                            .store(SessionStoreConfig::builder().store_type(store).build())
                            .build(),
                    )
                    .build(),
            )
            .build()
    }

    struct TestProject;

    impl Project for TestProject {
        fn register_apps(&self, _apps: &mut AppBuilder, _context: &RegisterAppsContext) {}
    }

    #[cot::test]
    async fn memory_store_factory_produces_working_store() {
        let config = create_project_config(SessionStoreTypeConfig::Memory);
        let bootstrapper = Bootstrapper::new(TestProject)
            .with_config(config)
            .with_apps()
            .with_database()
            .await
            .expect("bootstrap failed");
        let context = bootstrapper.context();

        create_svc_and_call_with_req(context).await;
    }

    #[cfg(feature = "json")]
    #[cot::test]
    async fn session_middleware_file_config_to_session_store() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path: PathBuf = dir.path().to_path_buf();
        let config = create_project_config(SessionStoreTypeConfig::File { path });

        let bootstrapper = Bootstrapper::new(TestProject)
            .with_config(config)
            .with_apps()
            .with_database()
            .await
            .expect("bootstrap failed");
        let context = bootstrapper.context();

        create_svc_and_call_with_req(context).await;
    }

    #[cfg(all(feature = "cache", feature = "redis"))]
    #[cot::test]
    #[ignore = "requires external Redis service"]
    async fn session_middleware_redis_config_to_session_store() {
        let redis_url =
            env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let uri = CacheUrl::from(redis_url);
        let config = create_project_config(SessionStoreTypeConfig::Cache { uri });
        let bootstrapper = Bootstrapper::new(TestProject)
            .with_config(config)
            .with_apps()
            .with_database()
            .await
            .expect("bootstrap failed");
        let context = bootstrapper.context();

        create_svc_and_call_with_req(context).await;
    }

    #[cfg(all(feature = "db", feature = "json"))]
    #[cot::test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
    )]
    async fn session_middleware_database_config_to_session_store() {
        let config = create_project_config(SessionStoreTypeConfig::Database);
        let bootstrapper = Bootstrapper::new(TestProject)
            .with_config(config)
            .with_apps()
            .with_database()
            .await
            .expect("bootstrap failed");
        let context = bootstrapper.context();

        create_svc_and_call_with_req(context).await;
    }
}
