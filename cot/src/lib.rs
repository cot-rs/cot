//! Cot is an easy to use, modern, and fast web framework for Rust. It has
//! been designed to be familiar if you've ever used
//! [Django](https://www.djangoproject.com/), and easy to learn if you haven't.
//! It's a batteries-included framework built on top of
//! [axum](https://github.com/tokio-rs/axum).
//!
//! ## Features
//!
//! * **Easy to use API** — in many ways modeled after Django, Cot's API is
//!   designed to be easy to use and intuitive. Sensible defaults make it for
//!   easy rapid development, while the API is still empowering you when needed.
//!   The documentation is a first-class citizen in Cot, making it easy to find
//!   what you're looking for.
//! * **ORM integration** — Cot comes with its own ORM, allowing you to interact
//!   with your database in a way that feels Rusty and intuitive. Rust types are
//!   the source of truth, and the ORM takes care of translating them to and
//!   from the database, as well as creating the migrations automatically.
//! * **Type safe** — wherever possible, Cot uses Rust's type system to prevent
//!   common mistakes and bugs. Not only views are taking advantage of the
//!   Rust's type system, but also the ORM, the admin panel, and even the
//!   templates. All that to catch errors as early as possible.
//! * **Admin panel** — Cot comes with an admin panel out of the box, allowing
//!   you to manage your app's data with ease. Adding new models to the admin
//!   panel is stupidly simple, making it a great tool not only for rapid
//!   development and debugging, but with its customization options, also for
//!   production use.
//! * **Secure by default** — security should be opt-out, not opt-in. Cot takes
//!   care of making your web apps secure by default, defending it against
//!   common modern web vulnerabilities. You can focus on building your app, not
//!   securing it.
//!
//! ## Guide
//!
//! This is an API reference for Cot, which might not be the best place to
//! start learning Cot. For a more gentle introduction, see the
//! [Cot guide](https://cot.rs/guide/latest/).
//!
//! ## Examples
//!
//! To see examples of how to use Cot, see the
//! [examples in the repository](https://github.com/cot-rs/cot/tree/master/examples).

#![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![cfg_attr(
    docsrs,
    feature(doc_cfg, doc_auto_cfg, rustdoc_missing_doc_code_examples)
)]
#![cfg_attr(docsrs, warn(rustdoc::missing_doc_code_examples))]

extern crate self as cot;

#[cfg(feature = "db")]
pub mod db;
mod error;
pub mod forms;
mod headers;
// Not public API. Referenced by macro-generated code.
#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;
pub mod admin;
pub mod auth;
pub mod cli;
pub mod config;
mod error_page;
pub mod middleware;
pub mod request;
pub mod response;
pub mod router;
pub mod static_files;
pub mod test;
pub(crate) mod utils;

use std::fmt::Formatter;
use std::future::{poll_fn, Future};
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use axum::handler::HandlerWithoutStateExt;
use bytes::Bytes;
pub use cot_macros::main;
use derive_more::{Debug, Deref, Display, From};
pub use error::Error;
use futures_core::Stream;
use futures_util::FutureExt;
use http::request::Parts;
use http_body::{Frame, SizeHint};
use http_body_util::combinators::BoxBody;
use request::Request;
use router::{Route, Router};
use sync_wrapper::SyncWrapper;
use tower::util::BoxCloneService;
use tower::{Layer, Service};
use tracing::info;
pub use {bytes, http};

use crate::admin::AdminModelManager;
use crate::cli::Cli;
#[cfg(feature = "db")]
use crate::config::DatabaseConfig;
use crate::config::ProjectConfig;
#[cfg(feature = "db")]
use crate::db::migrations::MigrationEngine;
#[cfg(feature = "db")]
use crate::db::migrations::SyncDynMigration;
#[cfg(feature = "db")]
use crate::db::Database;
use crate::error::ErrorRepr;
use crate::error_page::{CotDiagnostics, ErrorPageTrigger};
use crate::middleware::{IntoCotError, IntoCotErrorLayer, IntoCotResponse, IntoCotResponseLayer};
use crate::response::Response;
use crate::router::RouterService;

/// A type alias for a result that can return a [`cot::Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// A type alias for an HTTP status code.
pub type StatusCode = http::StatusCode;

/// A type alias for an HTTP method.
pub type Method = http::Method;

/// A function that takes a request and returns a response.
///
/// This is the main building block of a Cot app. You shouldn't
/// usually need to implement this directly, as it is already
/// implemented for closures and functions that take a [`Request`]
/// and return a [`Result<Response>`].
#[async_trait]
pub trait RequestHandler {
    async fn handle(&self, request: Request) -> Result<Response>;
}

#[async_trait]
impl<T, R> RequestHandler for T
where
    T: Fn(Request) -> R + Clone + Send + Sync + 'static,
    R: for<'a> Future<Output = Result<Response>> + Send,
{
    async fn handle(&self, request: Request) -> Result<Response> {
        self(request).await
    }
}

/// A building block for a Cot project.
///
/// A Cot app is a part (ideally, reusable) of a Cot project that is
/// responsible for its own set of functionalities. Examples of apps could be:
/// * admin panel
/// * user authentication
/// * blog
/// * message board
/// * session management
/// * etc.
///
/// Each app can have its own set of URLs that it can handle which can be
/// mounted on the project's router, its own set of middleware, database
/// migrations (which can depend on other apps), etc.
#[async_trait]
pub trait CotApp: Send + Sync {
    /// The name of the app.
    fn name(&self) -> &str;

    /// Initializes the app.
    ///
    /// This method is called when the app is initialized. It can be used to
    /// initialize whatever is needed for the app to work, possibly depending on
    /// other apps, or the project's configuration.
    ///
    /// # Errors
    ///
    /// This method returns an error if the app fails to initialize.
    #[allow(unused_variables)]
    async fn init(&self, context: &mut AppContext) -> Result<()> {
        Ok(())
    }

    /// Returns the router for the app. By default, it returns an empty router.
    fn router(&self) -> Router {
        Router::empty()
    }

    /// Returns the migrations for the app. By default, it returns an empty
    /// list.
    #[cfg(feature = "db")]
    fn migrations(&self) -> Vec<Box<SyncDynMigration>> {
        vec![]
    }

    /// Returns the admin model managers for the app. By default, it returns an
    /// empty list.
    fn admin_model_managers(&self) -> Vec<Box<dyn AdminModelManager>> {
        vec![]
    }

    /// Returns a list of static files that the app serves. By default, it
    /// returns an empty list.
    fn static_files(&self) -> Vec<(String, Bytes)> {
        vec![]
    }
}

/// A type that represents an HTTP request or response body.
///
/// This type is used to represent the body of an HTTP request/response. It can
/// be either a fixed body (e.g., a string or a byte array) or a streaming body
/// (e.g., a large file or a database query result).
///
/// # Examples
///
/// ```
/// use cot::Body;
///
/// let body = Body::fixed("Hello, world!");
/// let body = Body::streaming(futures::stream::once(async { Ok("Hello, world!".into()) }));
/// ```
#[derive(Debug)]
pub struct Body {
    inner: BodyInner,
}

enum BodyInner {
    Fixed(Bytes),
    Streaming(SyncWrapper<Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>>),
    Axum(SyncWrapper<axum::body::Body>),
    Wrapper(BoxBody<Bytes, Error>),
}

impl Debug for BodyInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fixed(data) => f.debug_tuple("Fixed").field(data).finish(),
            Self::Streaming(_) => f.debug_tuple("Streaming").field(&"...").finish(),
            Self::Axum(axum_body) => f.debug_tuple("Axum").field(axum_body).finish(),
            Self::Wrapper(_) => f.debug_tuple("Wrapper").field(&"...").finish(),
        }
    }
}

impl Body {
    #[must_use]
    const fn new(inner: BodyInner) -> Self {
        Self { inner }
    }

    /// Create an empty body.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Body;
    ///
    /// let body = Body::empty();
    /// ```
    #[must_use]
    pub const fn empty() -> Self {
        Self::new(BodyInner::Fixed(Bytes::new()))
    }

    /// Create a body instance with the given fixed data.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Body;
    ///
    /// let body = Body::fixed("Hello, world!");
    /// ```
    #[must_use]
    pub fn fixed<T: Into<Bytes>>(data: T) -> Self {
        Self::new(BodyInner::Fixed(data.into()))
    }

    /// Create a body instance from a stream of data.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_stream::stream;
    /// use cot::Body;
    ///
    /// let stream = stream! {
    ///    yield Ok("Hello, ".into());
    ///    yield Ok("world!".into());
    /// };
    /// let body = Body::streaming(stream);
    /// ```
    #[must_use]
    pub fn streaming<T: Stream<Item = Result<Bytes>> + Send + 'static>(stream: T) -> Self {
        Self::new(BodyInner::Streaming(SyncWrapper::new(Box::pin(stream))))
    }

    /// Convert this [`Body`] instance into a byte array.
    ///
    /// This method reads the entire body into memory and returns it as a byte
    /// array. Note that if the body is too large, this method can consume a lot
    /// of memory. For a way to read the body while limiting the memory usage,
    /// see [`Self::into_bytes_limited`].
    ///
    /// # Errors
    ///
    /// This method returns an error if reading the body fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Body;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let body = Body::fixed("Hello, world!");
    /// let bytes = body.into_bytes().await?;
    /// assert_eq!(bytes, "Hello, world!".as_bytes());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn into_bytes(self) -> std::result::Result<Bytes, Error> {
        self.into_bytes_limited(usize::MAX).await
    }

    /// Convert this [`Body`] instance into a byte array.
    ///
    /// This is a version of [`Self::into_bytes`] that allows you to limit the
    /// amount of memory used to read the body. If the body is larger than
    /// the limit, an error is returned.
    ///
    /// # Errors
    ///
    /// This method returns an error if reading the body fails.
    ///
    /// If the body is larger than the limit, an error is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Body;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let body = Body::fixed("Hello, world!");
    /// let bytes = body.into_bytes_limited(32).await?;
    /// assert_eq!(bytes, "Hello, world!".as_bytes());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn into_bytes_limited(self, limit: usize) -> std::result::Result<Bytes, Error> {
        use http_body_util::BodyExt;

        Ok(http_body_util::Limited::new(self, limit)
            .collect()
            .await
            .map(http_body_util::Collected::to_bytes)
            .map_err(|source| ErrorRepr::ReadRequestBody { source })?)
    }

    #[must_use]
    fn axum(inner: axum::body::Body) -> Self {
        Self::new(BodyInner::Axum(SyncWrapper::new(inner)))
    }

    #[must_use]
    pub(crate) fn wrapper(inner: BoxBody<Bytes, Error>) -> Self {
        Self::new(BodyInner::Wrapper(inner))
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::empty()
    }
}

impl http_body::Body for Body {
    type Data = Bytes;
    type Error = Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<std::result::Result<Frame<Self::Data>, Self::Error>>> {
        match self.get_mut().inner {
            BodyInner::Fixed(ref mut data) => {
                if data.is_empty() {
                    Poll::Ready(None)
                } else {
                    let data = std::mem::take(data);
                    Poll::Ready(Some(Ok(Frame::data(data))))
                }
            }
            BodyInner::Streaming(ref mut stream) => {
                let stream = Pin::as_mut(stream.get_mut());
                match stream.poll_next(cx) {
                    Poll::Ready(Some(result)) => Poll::Ready(Some(result.map(Frame::data))),
                    Poll::Ready(None) => Poll::Ready(None),
                    Poll::Pending => Poll::Pending,
                }
            }
            BodyInner::Axum(ref mut axum_body) => {
                let axum_body = axum_body.get_mut();
                Pin::new(axum_body).poll_frame(cx).map_err(|error| {
                    ErrorRepr::ReadRequestBody {
                        source: Box::new(error),
                    }
                    .into()
                })
            }
            BodyInner::Wrapper(ref mut http_body) => {
                Pin::new(http_body).poll_frame(cx).map_err(|error| {
                    ErrorRepr::ReadRequestBody {
                        source: Box::new(error),
                    }
                    .into()
                })
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        match &self.inner {
            BodyInner::Fixed(data) => data.is_empty(),
            BodyInner::Streaming(_) | BodyInner::Axum(_) => false,
            BodyInner::Wrapper(http_body) => http_body.is_end_stream(),
        }
    }

    fn size_hint(&self) -> SizeHint {
        match &self.inner {
            BodyInner::Fixed(data) => SizeHint::with_exact(data.len() as u64),
            BodyInner::Streaming(_) | BodyInner::Axum(_) => SizeHint::new(),
            BodyInner::Wrapper(http_body) => http_body.size_hint(),
        }
    }
}

/// A wrapper around a handler that's used in [`CotProject`].
///
/// It is returned by [`CotProject::into_context`]. Typically, you don't
/// need to interact with this type directly.
///
/// # Examples
///
/// ```
/// use cot::CotProject;
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let project = CotProject::builder().build().await?;
/// let (context, handler) = project.into_context();
/// # Ok(())
/// # }
/// ```
pub type BoxedHandler = BoxCloneService<Request, Response, Error>;

/// A Cot project, ready to be run.
#[derive(Debug)]
pub struct CotProject {
    context: AppContext,
    handler: BoxedHandler,
    cli: Cli,
}

/// A part of [`CotProject`] that contains the shared context and configs
/// for all apps.
#[derive(Debug)]
pub struct AppContext {
    config: Arc<ProjectConfig>,
    #[debug("...")]
    apps: Vec<Box<dyn CotApp>>,
    router: Arc<Router>,
    #[cfg(feature = "db")]
    database: Option<Arc<Database>>,
}

impl AppContext {
    #[must_use]
    pub(crate) fn new(
        config: Arc<ProjectConfig>,
        apps: Vec<Box<dyn CotApp>>,
        router: Arc<Router>,
        #[cfg(feature = "db")] database: Option<Arc<Database>>,
    ) -> Self {
        Self {
            config,
            apps,
            router,
            #[cfg(feature = "db")]
            database,
        }
    }

    /// Returns the configuration for the project.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    /// use cot::CotProject;
    ///
    /// async fn index(request: Request) -> cot::Result<Response> {
    ///     let config = request.context().config();
    ///     // can also be accessed via:
    ///     let config = request.project_config();
    ///
    ///     let db_url = config.database_config().url();
    ///
    ///     // ...
    /// #    todo!()
    /// }
    /// ```
    #[must_use]
    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }

    /// Returns the apps for the project.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    /// use cot::CotProject;
    ///
    /// async fn index(request: Request) -> cot::Result<Response> {
    ///     let apps = request.context().apps();
    ///
    ///     // ...
    /// #    todo!()
    /// }
    /// ```
    #[must_use]
    pub fn apps(&self) -> &[Box<dyn CotApp>] {
        &self.apps
    }

    /// Returns the router for the project.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    /// use cot::CotProject;
    ///
    /// async fn index(request: Request) -> cot::Result<Response> {
    ///     let router = request.context().config();
    ///     // can also be accessed via:
    ///     let router = request.router();
    ///
    ///     let num_routes = router.routes().len();
    ///
    ///     // ...
    /// #    todo!()
    /// }
    /// ```
    #[must_use]
    pub fn router(&self) -> &Router {
        &self.router
    }

    /// Returns the database for the project, if it is enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    /// use cot::CotProject;
    ///
    /// async fn index(request: Request) -> cot::Result<Response> {
    ///     let database = request.context().try_database();
    ///     if let Some(database) = database {
    ///         // do something with the database
    ///     } else {
    ///         // database is not enabled
    ///     }
    /// #    todo!()
    /// }
    /// ```
    #[must_use]
    #[cfg(feature = "db")]
    pub fn try_database(&self) -> Option<&Arc<Database>> {
        self.database.as_ref()
    }

    /// Returns the database for the project, if it is enabled.
    ///
    /// # Panics
    ///
    /// This method panics if the database is not enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    /// use cot::CotProject;
    ///
    /// async fn index(request: Request) -> cot::Result<Response> {
    ///     let database = request.context().database();
    ///     // can also be accessed via:
    ///     request.db();
    ///
    ///     // ...
    /// #    todo!()
    /// }
    /// ```
    #[must_use]
    #[cfg(feature = "db")]
    pub fn database(&self) -> &Database {
        self.try_database().expect(
            "Database missing. Did you forget to add the database when configuring CotProject?",
        )
    }
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone)]
pub struct Uninitialized;

/// The builder for the [`CotProject`].
#[derive(Debug)]
pub struct CotProjectBuilder<S> {
    context: AppContext,
    cli: Cli,
    urls: Vec<Route>,
    handler: S,
}

impl CotProjectBuilder<Uninitialized> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            context: AppContext {
                config: Arc::new(ProjectConfig::default()),
                apps: vec![],
                router: Arc::new(Router::default()),
                #[cfg(feature = "db")]
                database: None,
            },
            cli: Cli::new(),
            urls: Vec::new(),
            handler: Uninitialized,
        }
    }

    /// Sets the metadata for the CLI.
    ///
    /// This method is used to set the name, version, authors, and description
    /// of the CLI application. This is meant to be typically used with
    /// [`crate::cli::metadata!`].
    #[must_use]
    pub fn with_cli(mut self, metadata: cli::CliMetadata) -> Self {
        self.cli.set_metadata(metadata);
        self
    }

    /// Adds a task to the CLI.
    ///
    /// This method is used to add a task to the CLI. The task will be available
    /// as a subcommand of the main CLI command.
    #[must_use]
    pub fn add_task<C>(mut self, task: C) -> Self
    where
        C: cli::CliTask + Send + 'static,
    {
        self.cli.add_task(task);
        self
    }

    #[must_use]
    pub fn config(mut self, config: ProjectConfig) -> Self {
        self.context.config = Arc::new(config);
        self
    }

    #[must_use]
    pub fn register_app_with_views<T: CotApp + 'static>(
        mut self,
        app: T,
        url_prefix: &str,
    ) -> Self {
        self.urls.push(Route::with_router(url_prefix, app.router()));
        self = self.register_app(app);
        self
    }

    #[must_use]
    pub fn register_app<T: CotApp + 'static>(mut self, app: T) -> Self {
        self.context.apps.push(Box::new(app));
        self
    }

    #[must_use]
    pub fn middleware<M>(
        self,
        middleware: M,
    ) -> CotProjectBuilder<IntoCotError<IntoCotResponse<M::Service>>>
    where
        M: Layer<RouterService>,
    {
        self.into_builder_with_service().middleware(middleware)
    }

    #[must_use]
    pub fn middleware_with_context<M, F>(
        self,
        get_middleware: F,
    ) -> CotProjectBuilder<IntoCotError<IntoCotResponse<M::Service>>>
    where
        M: Layer<RouterService>,
        F: FnOnce(&AppContext) -> M,
    {
        self.into_builder_with_service()
            .middleware_with_context(get_middleware)
    }

    /// Builds the Cot project instance.
    pub async fn build(self) -> Result<CotProject> {
        self.into_builder_with_service().build().await
    }

    #[must_use]
    fn into_builder_with_service(mut self) -> CotProjectBuilder<RouterService> {
        let router = Arc::new(Router::with_urls(self.urls));
        self.context.router = Arc::clone(&router);

        CotProjectBuilder {
            context: self.context,
            cli: self.cli,
            urls: vec![],
            handler: RouterService::new(router),
        }
    }
}

impl<S> CotProjectBuilder<S>
where
    S: Service<Request, Response = Response, Error = Error> + Send + Sync + Clone + 'static,
    S::Future: Send,
{
    #[must_use]
    pub fn middleware<M>(
        self,
        middleware: M,
    ) -> CotProjectBuilder<IntoCotError<IntoCotResponse<<M as Layer<S>>::Service>>>
    where
        M: Layer<S>,
    {
        let layer = (
            IntoCotErrorLayer::new(),
            IntoCotResponseLayer::new(),
            middleware,
        );

        CotProjectBuilder {
            context: self.context,
            cli: self.cli,
            urls: vec![],
            handler: layer.layer(self.handler),
        }
    }

    #[must_use]
    pub fn middleware_with_context<M, F>(
        self,
        get_middleware: F,
    ) -> CotProjectBuilder<IntoCotError<IntoCotResponse<<M as Layer<S>>::Service>>>
    where
        M: Layer<S>,
        F: FnOnce(&AppContext) -> M,
    {
        let middleware = get_middleware(&self.context);
        self.middleware(middleware)
    }

    /// Builds the Cot project instance.
    pub async fn build(mut self) -> Result<CotProject> {
        #[cfg(feature = "db")]
        {
            let database = Self::init_database(self.context.config.database_config()).await?;
            self.context.database = Some(database);
        }

        Ok(CotProject {
            context: self.context,
            cli: self.cli,
            handler: BoxedHandler::new(self.handler),
        })
    }

    #[cfg(feature = "db")]
    async fn init_database(config: &DatabaseConfig) -> Result<Arc<Database>> {
        let database = Database::new(config.url()).await?;
        Ok(Arc::new(database))
    }
}

impl Default for CotProjectBuilder<Uninitialized> {
    fn default() -> Self {
        Self::new()
    }
}

impl CotProject {
    #[must_use]
    pub fn builder() -> CotProjectBuilder<Uninitialized> {
        CotProjectBuilder::default()
    }

    #[must_use]
    pub fn into_context(self) -> (AppContext, BoxedHandler) {
        (self.context, self.handler)
    }
}

/// Runs the Cot project on the given address.
///
/// This function takes a Cot project and an address string and runs the
/// project on the given address.
///
/// # Errors
///
/// This function returns an error if the server fails to start.
pub async fn run(project: CotProject, address_str: &str) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(address_str)
        .await
        .map_err(|e| ErrorRepr::StartServer { source: e })?;

    run_at(project, listener).await
}

/// Runs the Cot project on the given listener.
///
/// This function takes a Cot project and a [`tokio::net::TcpListener`] and
/// runs the project on the given listener.
///
/// If you need more control over the server listening socket, such as modifying
/// the underlying buffer sizes, you can create a [`tokio::net::TcpListener`]
/// and pass it to this function. Otherwise, [`run`] function will be more
/// convenient.
///
/// # Errors
///
/// This function returns an error if the server fails to start.
pub async fn run_at(project: CotProject, listener: tokio::net::TcpListener) -> Result<()> {
    let (mut context, mut project_handler) = project.into_context();

    #[cfg(feature = "db")]
    if let Some(database) = &context.database {
        let mut migrations: Vec<Box<SyncDynMigration>> = Vec::new();
        for app in &context.apps {
            migrations.extend(app.migrations());
        }
        let migration_engine = MigrationEngine::new(migrations)?;
        migration_engine.run(database).await?;
    }

    let mut apps = std::mem::take(&mut context.apps);
    for app in &mut apps {
        info!("Initializing app: {}", app.name());

        app.init(&mut context).await?;
    }
    context.apps = apps;

    let context = Arc::new(context);
    #[cfg(feature = "db")]
    let context_cleanup = context.clone();

    let handler = |axum_request: axum::extract::Request| async move {
        let request = request_axum_to_cot(axum_request, Arc::clone(&context));
        let (request_parts, request) = request_parts_for_diagnostics(request);

        let catch_unwind_response = AssertUnwindSafe(pass_to_axum(request, &mut project_handler))
            .catch_unwind()
            .await;

        let show_error_page = match &catch_unwind_response {
            Ok(response) => match response {
                Ok(response) => response.extensions().get::<ErrorPageTrigger>().is_some(),
                Err(_) => true,
            },
            Err(_) => {
                // handler panicked
                true
            }
        };

        if show_error_page {
            let diagnostics = CotDiagnostics::new(Arc::clone(&context.router), request_parts);

            match catch_unwind_response {
                Ok(response) => match response {
                    Ok(response) => match response
                        .extensions()
                        .get::<ErrorPageTrigger>()
                        .expect("ErrorPageTrigger already has been checked to be Some")
                    {
                        ErrorPageTrigger::NotFound => error_page::handle_not_found(diagnostics),
                    },
                    Err(error) => error_page::handle_response_error(error, diagnostics),
                },
                Err(error) => error_page::handle_response_panic(error, diagnostics),
            }
        } else {
            catch_unwind_response
                .expect("Error page should be shown if the response is not a panic")
                .expect("Error page should be shown if the response is not an error")
        }
    };

    eprintln!(
        "Starting the server at http://{}",
        listener
            .local_addr()
            .map_err(|e| ErrorRepr::StartServer { source: e })?
    );
    if config::REGISTER_PANIC_HOOK {
        let current_hook = std::panic::take_hook();
        let new_hook = move |hook_info: &std::panic::PanicHookInfo<'_>| {
            current_hook(hook_info);
            error_page::error_page_panic_hook(hook_info);
        };
        std::panic::set_hook(Box::new(new_hook));
    }
    axum::serve(listener, handler.into_make_service())
        .await
        .map_err(|e| ErrorRepr::StartServer { source: e })?;
    if config::REGISTER_PANIC_HOOK {
        let _ = std::panic::take_hook();
    }
    #[cfg(feature = "db")]
    if let Some(database) = &context_cleanup.database {
        database.close().await?;
    }

    Ok(())
}

/// Runs the CLI for the given project.
///
/// This function takes a [`CotProject`] and runs the CLI for the project. You
/// typically don't need to call this function directly. Instead, you can use
/// [`cot::main`] which is a more ergonomic way to run the CLI.
///
/// # Errors
///
/// This function returns an error if the CLI command fails to execute.
///
/// # Examples
///
/// ```no_run
/// use cot::{run_cli, CotProject};
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let project = CotProject::builder().build().await?;
/// run_cli(project).await?;
/// # Ok(())
/// # }
/// ```
pub async fn run_cli(mut project: CotProject) -> Result<()> {
    std::mem::take(&mut project.cli).execute(project).await
}

fn request_parts_for_diagnostics(request: Request) -> (Option<Parts>, Request) {
    if config::DEBUG_MODE {
        let (parts, body) = request.into_parts();
        let parts_clone = parts.clone();
        let request = Request::from_parts(parts, body);
        (Some(parts_clone), request)
    } else {
        (None, request)
    }
}

fn request_axum_to_cot(axum_request: axum::extract::Request, context: Arc<AppContext>) -> Request {
    let mut request = axum_request.map(Body::axum);
    prepare_request(&mut request, context);
    request
}

pub(crate) fn prepare_request(request: &mut Request, context: Arc<AppContext>) {
    request.extensions_mut().insert(context);
}

async fn pass_to_axum(
    request: Request,
    handler: &mut BoxedHandler,
) -> Result<axum::response::Response> {
    poll_fn(|cx| handler.poll_ready(cx)).await?;
    let response = handler.call(request).await?;

    Ok(response.map(axum::body::Body::new))
}

/// A trait for types that can be used to render them as HTML.
pub trait Render {
    /// Renders the object as an HTML string.
    fn render(&self) -> Html;
}

/// A type that represents HTML content as a string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deref, From, Display)]
pub struct Html(String);

impl Html {
    #[must_use]
    pub fn new<T: Into<String>>(html: T) -> Self {
        Self(html.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use futures::stream;
    use http_body::Body as HttpBody;

    use super::*;

    struct MockCotApp;

    impl CotApp for MockCotApp {
        fn name(&self) -> &'static str {
            "mock"
        }
    }

    #[tokio::test]
    async fn cot_app_default_impl() {
        let app = MockCotApp {};
        assert_eq!(app.name(), "mock");
        assert_eq!(app.router().routes().len(), 0);
        assert_eq!(app.migrations().len(), 0);
    }

    #[tokio::test]
    #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `sqlite3_open_v2`
    async fn cot_project_builder() {
        let project = CotProject::builder()
            .register_app_with_views(MockCotApp {}, "/app")
            .build()
            .await
            .unwrap();
        assert_eq!(project.context.apps.len(), 1);
        assert!(!project.context.router.is_empty());
    }

    #[tokio::test]
    #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `sqlite3_open_v2`
    async fn cot_project_router() {
        let project = CotProject::builder()
            .register_app_with_views(MockCotApp {}, "/app")
            .build()
            .await
            .unwrap();
        assert_eq!(project.context.router.routes().len(), 1);
    }

    #[test]
    fn body_empty() {
        let body = Body::empty();
        if let BodyInner::Fixed(data) = body.inner {
            assert!(data.is_empty());
        } else {
            panic!("Body::empty should create a fixed empty body");
        }
    }

    #[test]
    fn body_fixed() {
        let content = "Hello, world!";
        let body = Body::fixed(content);
        if let BodyInner::Fixed(data) = body.inner {
            assert_eq!(data, Bytes::from(content));
        } else {
            panic!("Body::fixed should create a fixed body with the given content");
        }
    }

    #[tokio::test]
    async fn body_streaming() {
        let stream = stream::once(async { Ok(Bytes::from("Hello, world!")) });
        let body = Body::streaming(stream);
        if let BodyInner::Streaming(_) = body.inner {
            // Streaming body created successfully
        } else {
            panic!("Body::streaming should create a streaming body");
        }
    }

    #[tokio::test]
    async fn http_body_poll_frame_fixed() {
        let content = "Hello, world!";
        let mut body = Body::fixed(content);
        let mut cx = Context::from_waker(futures::task::noop_waker_ref());

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                assert_eq!(frame.into_data().unwrap(), Bytes::from(content));
            }
            _ => panic!("Body::fixed should return the content in poll_frame"),
        }

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => {} // End of stream
            _ => panic!("Body::fixed should return None after the content is consumed"),
        }
    }

    #[tokio::test]
    async fn http_body_poll_frame_streaming() {
        let content = "Hello, world!";
        let mut body = Body::streaming(stream::once(async move { Ok(Bytes::from(content)) }));
        let mut cx = Context::from_waker(futures::task::noop_waker_ref());

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                assert_eq!(frame.into_data().unwrap(), Bytes::from(content));
            }
            _ => panic!("Body::fixed should return the content in poll_frame"),
        }

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => {} // End of stream
            _ => panic!("Body::fixed should return None after the content is consumed"),
        }
    }

    #[test]
    fn http_body_is_end_stream() {
        let body = Body::empty();
        assert!(body.is_end_stream());

        let body = Body::fixed("Hello, world!");
        assert!(!body.is_end_stream());
    }

    #[test]
    fn http_body_size_hint() {
        let body = Body::empty();
        assert_eq!(body.size_hint().exact(), Some(0));

        let content = "Hello, world!";
        let body = Body::fixed(content);
        assert_eq!(body.size_hint().exact(), Some(content.len() as u64));
    }
}
