//! This module contains the core types and traits for a Cot project.
//!
//! This module defines the [`Project`] and [`App`] traits, which are the main
//! entry points for your application.
//! # Examples
//!
//! ```no_run
//! use cot::Project;
//! use cot::cli::CliMetadata;
//!
//! struct MyProject;
//! impl Project for MyProject {
//!     fn cli_metadata(&self) -> CliMetadata {
//!         cot::cli::metadata!()
//!     }
//! }
//!
//! #[cot::main]
//! fn main() -> impl Project {
//!     MyProject
//! }
//! ```
use std::future::poll_fn;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;

use askama::Template;
use async_trait::async_trait;
use axum::handler::HandlerWithoutStateExt;
use derive_more::with_trait::Debug;
use futures_util::FutureExt;
use http::request::Parts;
use tower::util::BoxCloneSyncService;
use tower::{Layer, Service};
use tracing::{error, info, trace};

use crate::admin::AdminModelManager;
#[cfg(feature = "db")]
use crate::auth::db::DatabaseUserBackend;
use crate::auth::{AuthBackend, NoAuthBackend};
use crate::cli::Cli;
#[cfg(feature = "db")]
use crate::config::DatabaseConfig;
use crate::config::{AuthBackendConfig, ProjectConfig};
#[cfg(feature = "db")]
use crate::db::Database;
#[cfg(feature = "db")]
use crate::db::migrations::{MigrationEngine, SyncDynMigration};
use crate::error::error_impl::ErrorKind;
use crate::error::handler::{DynErrorPageHandler, RequestError};
use crate::error::uncaught_panic::UncaughtPanic;
use crate::error_page::Diagnostics;
use crate::handler::BoxedHandler;
use crate::headers::HTML_NO_CHARSET_CONTENT_TYPE;
use crate::html::Html;
use crate::middleware::{IntoCotError, IntoCotErrorLayer, IntoCotResponse, IntoCotResponseLayer};
use crate::request::{AppName, Request, RequestExt};
use crate::response::{IntoResponse, Response};
use crate::router::{Route, Router, RouterService};
use crate::static_files::StaticFile;
use crate::{Body, Error, cli, error_page};

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
pub trait App: Send + Sync {
    /// The name of the app.
    ///
    /// This should usually be the name of the crate.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::App;
    ///
    /// struct MyApp;
    /// impl App for MyApp {
    ///     fn name(&self) -> &str {
    ///         env!("CARGO_PKG_NAME")
    ///     }
    /// }
    /// ```
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
    #[expect(unused_variables)]
    async fn init(&self, context: &mut ProjectContext) -> crate::Result<()> {
        Ok(())
    }

    /// Returns the router for the app. By default, it returns an empty router.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::App;
    /// use cot::html::Html;
    /// use cot::router::{Route, Router};
    ///
    /// async fn index() -> Html {
    ///     Html::new("Hello world!")
    /// }
    ///
    /// struct MyApp;
    /// impl App for MyApp {
    ///     fn name(&self) -> &str {
    ///         "my_app"
    ///     }
    ///
    ///     fn router(&self) -> Router {
    ///         Router::with_urls([Route::with_handler("/", index)])
    ///     }
    /// }
    /// ```
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
    fn static_files(&self) -> Vec<StaticFile> {
        vec![]
    }
}

/// The main trait for a Cot project.
///
/// This is the main entry point for your application. This trait defines
/// the configuration, apps, and other project-wide resources.
///
/// It's mainly meant to be used with the [`cot::main`] attribute macro.
///
/// # Examples
///
/// ```no_run
/// use cot::Project;
/// use cot::cli::CliMetadata;
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn cli_metadata(&self) -> CliMetadata {
///         cot::cli::metadata!()
///     }
/// }
///
/// #[cot::main]
/// fn main() -> impl Project {
///     MyProject
/// }
/// ```
pub trait Project {
    /// Returns the metadata for the CLI.
    ///
    /// This method is used to set the name, version, authors, and description
    /// of the CLI application. This is meant to be typically used with
    /// [`cli::metadata!()`] which automatically retrieves this data from the
    /// crate metadata.
    ///
    /// The default implementation sets the name, version, authors, and
    /// description of the `cot` crate.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Project;
    /// use cot::cli::CliMetadata;
    ///
    /// struct HelloProject;
    /// impl Project for HelloProject {
    ///     fn cli_metadata(&self) -> CliMetadata {
    ///         cot::cli::metadata!()
    ///     }
    /// }
    /// ```
    fn cli_metadata(&self) -> cli::CliMetadata {
        cli::metadata!()
    }

    /// Returns the configuration for the project.
    ///
    /// The default implementation reads the configuration from the `config`
    /// directory in the current working directory (for instance, if
    /// `config_name` is `test`, then `config/test.toml` in the current working
    /// directory is read). If the file does not exist, it tries to read the
    /// file directly at `config_name` path.
    ///
    /// You might want to override this method if you want to read the
    /// configuration from a different source, or if you want to hardcode
    /// it in the binary.
    ///
    /// # Errors
    ///
    /// This method may return an error if it cannot read or parse the
    /// configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Project;
    /// use cot::config::ProjectConfig;
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn config(&self, config_name: &str) -> cot::Result<ProjectConfig> {
    ///         Ok(ProjectConfig::default())
    ///     }
    /// }
    /// ```
    fn config(&self, config_name: &str) -> crate::Result<ProjectConfig> {
        read_config(config_name)
    }

    /// Adds a task to the CLI.
    ///
    /// This method is used to add a task to the CLI. The task will be available
    /// as a subcommand of the main CLI command.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_trait::async_trait;
    /// use clap::{ArgMatches, Command};
    /// use cot::cli::{Cli, CliTask};
    /// use cot::project::WithConfig;
    /// use cot::{Bootstrapper, Project};
    ///
    /// struct Frobnicate;
    ///
    /// #[async_trait(?Send)]
    /// impl CliTask for Frobnicate {
    ///     fn subcommand(&self) -> Command {
    ///         Command::new("frobnicate")
    ///     }
    ///
    ///     async fn execute(
    ///         &mut self,
    ///         _matches: &ArgMatches,
    ///         _bootstrapper: Bootstrapper<WithConfig>,
    ///     ) -> cot::Result<()> {
    ///         println!("Frobnicating...");
    ///
    ///         Ok(())
    ///     }
    /// }
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn register_tasks(&self, cli: &mut Cli) {
    ///         cli.add_task(Frobnicate)
    ///     }
    /// }
    /// ```
    #[expect(unused_variables)]
    fn register_tasks(&self, cli: &mut Cli) {}

    /// Registers the apps for the project.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::project::{AppBuilder, RegisterAppsContext};
    /// use cot::{App, Project};
    ///
    /// struct MyApp;
    /// impl App for MyApp {
    ///     fn name(&self) -> &str {
    ///         "my_app"
    ///     }
    /// }
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn register_apps(&self, apps: &mut AppBuilder, context: &RegisterAppsContext) {
    ///         apps.register(MyApp);
    ///     }
    /// }
    /// ```
    #[expect(unused_variables)]
    fn register_apps(&self, apps: &mut AppBuilder, context: &RegisterAppsContext) {}

    /// Sets the authentication backend to use.
    ///
    /// Note that it's typically not necessary to override this method, as it
    /// already provides a default implementation that uses the auth backend
    /// specified in the project's configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use cot::auth::{AuthBackend, NoAuthBackend};
    /// use cot::project::AuthBackendContext;
    /// use cot::{App, Project};
    ///
    /// struct HelloProject;
    /// impl Project for HelloProject {
    ///     fn auth_backend(&self, context: &AuthBackendContext) -> Arc<dyn AuthBackend> {
    ///         Arc::new(NoAuthBackend)
    ///     }
    /// }
    /// ```
    fn auth_backend(&self, context: &AuthBackendContext) -> Arc<dyn AuthBackend> {
        #[expect(trivial_casts)] // cast to Arc<dyn AuthBackend>
        match &context.config().auth_backend {
            AuthBackendConfig::None => Arc::new(NoAuthBackend) as Arc<dyn AuthBackend>,
            #[cfg(feature = "db")]
            AuthBackendConfig::Database => Arc::new(DatabaseUserBackend::new(
                context
                    .try_database()
                    .expect(
                        "Database missing when constructing database auth backend. \
                        Make sure the database config is set up correctly or disable \
                        authentication in the config.",
                    )
                    .clone(),
            )) as Arc<dyn AuthBackend>,
        }
    }

    /// Returns the middlewares for the project.
    ///
    /// This method is used to return the middlewares for the project. The
    /// middlewares will be applied to all routes in the project.
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
    ///             .middleware(LiveReloadMiddleware::from_context(context))
    ///             .build()
    ///     }
    /// }
    /// ```
    #[expect(unused_variables)]
    fn middlewares(&self, handler: RootHandlerBuilder, context: &MiddlewareContext) -> RootHandler {
        handler.build()
    }

    /// Returns the error handler for the project.
    ///
    /// The default handler returns a simple, minimalistic error page
    /// that displays the status code canonical name and a
    /// generic error message.
    ///
    /// # Errors
    ///
    /// This method may return an error if the handler fails to build a
    /// response. In this case, the error will be logged and a generic
    /// error page will be returned to the user.
    ///
    /// # Panics
    ///
    /// Note that this handler is exempt of the typical panic handling
    /// machinery in Cot. This means that if this handler panics, no
    /// response will be sent to a user. Because of that, you should
    /// avoid panicking here and return [`Err`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::error::handler::{DynErrorPageHandler, ErrorPageHandler};
    /// use cot::html::Html;
    /// use cot::response::{IntoResponse, Response};
    /// use cot::{Error, Project, Result, StatusCode};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn server_error_handler(&self) -> DynErrorPageHandler {
    ///         DynErrorPageHandler::new(error_handler)
    ///     }
    /// }
    ///
    /// async fn error_handler(error: Error) -> impl IntoResponse {
    ///     Html::new(format!("An error occurred: {error}")).with_status(error.status_code())
    /// }
    /// ```
    fn server_error_handler(&self) -> DynErrorPageHandler {
        DynErrorPageHandler::new(default_server_error_handler)
    }
}

/// An alias for `ProjectContext` in appropriate phase for use with the
/// [`Project::register_apps`] method.
pub type RegisterAppsContext = ProjectContext<WithConfig>;

/// An alias for `ProjectContext` in appropriate phase for use with the
/// [`Project::auth_backend`] method.
pub type AuthBackendContext = ProjectContext<WithDatabase>;

/// An alias for `ProjectContext` in appropriate phase for use with the
/// [`Project::middlewares`] method.
pub type MiddlewareContext = ProjectContext<WithDatabase>;

/// A helper struct to build the root handler for the project.
///
/// This is mainly useful for attaching middlewares to the project.
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
///             .middleware(LiveReloadMiddleware::from_context(context))
///             .build()
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RootHandlerBuilder<S = RouterService, SE = DynErrorPageHandler> {
    handler: S,
    error_handler: SE,
}

impl<RootService, ErrorHandlerService> RootHandlerBuilder<RootService, ErrorHandlerService>
where
    RootService:
        Service<Request, Response = Response, Error = Error> + Send + Sync + Clone + 'static,
    RootService::Future: Send,
    ErrorHandlerService:
        Service<Request, Response = Response, Error = Error> + Send + Sync + Clone + 'static,
    ErrorHandlerService::Future: Send,
{
    /// Adds middleware to the project.
    ///
    /// This method is used to add middleware to the project. The middleware
    /// will be applied to all routes in the project.
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
    ///             .middleware(LiveReloadMiddleware::from_context(context))
    ///             .build()
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn middleware<M>(
        self,
        middleware: M,
    ) -> RootHandlerBuilder<
        IntoCotError<IntoCotResponse<<M as Layer<RootService>>::Service>>,
        IntoCotError<IntoCotResponse<<M as Layer<ErrorHandlerService>>::Service>>,
    >
    where
        M: Layer<RootService> + Layer<ErrorHandlerService>,
    {
        let layer = (
            IntoCotErrorLayer::new(),
            IntoCotResponseLayer::new(),
            middleware,
        );

        RootHandlerBuilder {
            handler: layer.layer(self.handler),
            error_handler: layer.layer(self.error_handler),
        }
    }

    /// Adds middleware to only the main request handler.
    ///
    /// This method is used to add middleware that should only be applied to
    /// the main request handler, not to the error handler. This is useful when
    /// you have middleware that should only run for successful requests, such
    /// as logging middleware that should not interfere with error handling.
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
    ///             .main_handler_middleware(LiveReloadMiddleware::from_context(context))
    ///             .build()
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn main_handler_middleware<M>(
        self,
        middleware: M,
    ) -> RootHandlerBuilder<
        IntoCotError<IntoCotResponse<<M as Layer<RootService>>::Service>>,
        ErrorHandlerService,
    >
    where
        M: Layer<RootService>,
    {
        let layer = (
            IntoCotErrorLayer::new(),
            IntoCotResponseLayer::new(),
            middleware,
        );

        RootHandlerBuilder {
            handler: layer.layer(self.handler),
            error_handler: self.error_handler,
        }
    }

    /// Adds middleware to only the error handler.
    ///
    /// This method is used to add middleware that should only be applied to
    /// the error handler, not to the main request handler. This is useful when
    /// you have middleware that should only run when handling errors, such as
    /// error reporting middleware or middleware that modifies error responses.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Project;
    /// use cot::project::{MiddlewareContext, RootHandler, RootHandlerBuilder};
    /// use cot::static_files::StaticFilesMiddleware;
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn middlewares(
    ///         &self,
    ///         handler: RootHandlerBuilder,
    ///         context: &MiddlewareContext,
    ///     ) -> RootHandler {
    ///         handler
    ///             .error_handler_middleware(StaticFilesMiddleware::from_context(context))
    ///             .build()
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn error_handler_middleware<M>(
        self,
        middleware: M,
    ) -> RootHandlerBuilder<
        RootService,
        IntoCotError<IntoCotResponse<<M as Layer<ErrorHandlerService>>::Service>>,
    >
    where
        M: Layer<ErrorHandlerService>,
    {
        let layer = (
            IntoCotErrorLayer::new(),
            IntoCotResponseLayer::new(),
            middleware,
        );

        RootHandlerBuilder {
            handler: self.handler,
            error_handler: layer.layer(self.error_handler),
        }
    }

    /// Builds the root handler for the project.
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
    ///             .middleware(LiveReloadMiddleware::from_context(context))
    ///             .build()
    ///     }
    /// }
    /// ```
    pub fn build(self) -> RootHandler {
        RootHandler {
            handler: BoxedHandler::new(self.handler),
            error_handler: BoxedHandler::new(self.error_handler),
        }
    }
}

/// A built root handler that contains both the main request handler and error
/// handler.
///
/// This struct is the result of building a [`RootHandlerBuilder`] and contains
/// the finalized handlers that are ready to be used for processing requests
/// and handling errors.
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
///             .middleware(LiveReloadMiddleware::from_context(context))
///             .build()
///     }
/// }
/// ```
#[derive(Debug)]
pub struct RootHandler {
    /// The main request handler that processes incoming requests.
    pub(crate) handler: BoxedHandler,
    /// The error handler that processes errors that occur during request
    /// handling.
    pub(crate) error_handler: BoxedHandler,
}

/// A helper struct to build the apps for the project.
///
/// # Examples
///
/// ```
/// use cot::project::{AppBuilder, RegisterAppsContext};
/// use cot::{App, Project};
///
/// struct MyApp;
/// impl App for MyApp {
///     fn name(&self) -> &str {
///         "my_app"
///     }
/// }
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn register_apps(&self, apps: &mut AppBuilder, context: &RegisterAppsContext) {
///         apps.register(MyApp);
///     }
/// }
/// ```
#[derive(Debug)]
pub struct AppBuilder {
    #[debug("..")]
    apps: Vec<Box<dyn App>>,
    urls: Vec<Route>,
}

impl AppBuilder {
    fn new() -> Self {
        Self {
            apps: Vec::new(),
            urls: Vec::new(),
        }
    }

    /// Registers an app.
    ///
    /// This method is used to register an app. The app's views, if any, will
    /// not be available.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::project::RegisterAppsContext;
    /// use cot::{App, Project};
    ///
    /// struct HelloApp;
    ///
    /// impl App for HelloApp {
    ///     fn name(&self) -> &'static str {
    ///         env!("CARGO_PKG_NAME")
    ///     }
    /// }
    ///
    /// struct HelloProject;
    /// impl Project for HelloProject {
    ///     fn register_apps(&self, apps: &mut cot::AppBuilder, _context: &RegisterAppsContext) {
    ///         apps.register(HelloApp);
    ///     }
    /// }
    /// ```
    pub fn register<T: App + 'static>(&mut self, module: T) {
        self.apps.push(Box::new(module));
    }

    /// Registers an app with views.
    ///
    /// This method is used to register an app with views. The app's views will
    /// be available at the given URL prefix.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::project::RegisterAppsContext;
    /// use cot::{App, Project};
    ///
    /// struct HelloApp;
    ///
    /// impl App for HelloApp {
    ///     fn name(&self) -> &'static str {
    ///         env!("CARGO_PKG_NAME")
    ///     }
    /// }
    ///
    /// struct HelloProject;
    /// impl Project for HelloProject {
    ///     fn register_apps(&self, apps: &mut cot::AppBuilder, _context: &RegisterAppsContext) {
    ///         apps.register_with_views(HelloApp, "/hello");
    ///     }
    /// }
    /// ```
    pub fn register_with_views<T: App + 'static>(&mut self, app: T, url_prefix: &str) {
        let mut router = app.router();
        router.set_app_name(AppName(app.name().to_owned()));

        self.urls.push(Route::with_router(url_prefix, router));
        self.register(app);
    }
}

async fn default_server_error_handler(error: Error) -> cot::Result<impl IntoResponse> {
    #[derive(Debug, Template)]
    #[template(path = "default_error.html")]
    struct ErrorTemplate {
        error: Error,
    }

    let status_code = error.status_code();
    let error_template = ErrorTemplate { error };
    let rendered = error_template.render()?;

    Ok(Html::new(rendered).with_status(status_code))
}

/// The main struct for bootstrapping the project.
///
/// This is the core struct for bootstrapping the project. It goes over the
/// different phases of bootstrapping the project which are defined in the
/// [`BootstrapPhase`] trait. Each phase has its own subset of the project's
/// context that is available, and you have access to specific parts of the
/// project's context depending where you are in the bootstrapping process.
///
/// Note that you shouldn't have to use this struct directly most of the time.
/// It's mainly used internally by the `cot` crate to bootstrap the project.
/// It can be useful if you want to control the bootstrapping process in
/// custom [`CliTask`](cli::CliTask)s.
///
/// # Examples
///
/// ```
/// use cot::project::{Bootstrapper, WithConfig};
/// use cot::{App, Project};
///
/// struct MyProject;
/// impl Project for MyProject {}
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let bootstrapper = Bootstrapper::new(MyProject)
///     .with_config(cot::config::ProjectConfig::default())
///     .boot()
///     .await?;
/// let bootstrapped_project = bootstrapper.into_bootstrapped_project();
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Bootstrapper<S: BootstrapPhase = Initialized> {
    #[debug("..")]
    project: Box<dyn Project>,
    context: ProjectContext<S>,
    handler: S::RequestHandler,
    error_handler: S::ErrorHandler,
}

impl Bootstrapper<Uninitialized> {
    /// Creates a new bootstrapper.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::project::{Bootstrapper, WithConfig};
    /// use cot::{App, Project};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {}
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let bootstrapper = Bootstrapper::new(MyProject);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn new<P: Project + 'static>(project: P) -> Self {
        Self {
            project: Box::new(project),
            context: ProjectContext::new(),
            handler: (),
            error_handler: (),
        }
    }
}

impl<S: BootstrapPhase> Bootstrapper<S> {
    /// Returns the project for the bootstrapper.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::project::{Bootstrapper, WithConfig};
    /// use cot::{App, Project};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {}
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let bootstrapper = Bootstrapper::new(MyProject);
    /// let project = bootstrapper.project();
    /// # Ok(())
    /// # }
    /// ```
    pub fn project(&self) -> &dyn Project {
        self.project.as_ref()
    }

    /// Returns the context for the bootstrapper.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::project::{Bootstrapper, WithConfig};
    /// use cot::{App, Project};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {}
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let bootstrapper = Bootstrapper::new(MyProject);
    /// let context = bootstrapper.context();
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn context(&self) -> &ProjectContext<S> {
        &self.context
    }
}

impl Bootstrapper<Uninitialized> {
    #[expect(clippy::future_not_send)] // Send not needed; CLI is run async in a single thread
    async fn run_cli(self) -> cot::Result<()> {
        let mut cli = Cli::new();

        cli.set_metadata(self.project.cli_metadata());
        self.project.register_tasks(&mut cli);

        let common_options = cli.common_options();
        let self_with_context = self.with_config_name(common_options.config())?;

        cli.execute(self_with_context).await
    }

    /// Reads the configuration of the project and moves to the next
    /// bootstrapping phase.
    ///
    /// # Errors
    ///
    /// This method may return an error if it cannot read the configuration of
    /// the project.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    /// use cot::{Bootstrapper, Project};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn config(&self, config_name: &str) -> cot::Result<ProjectConfig> {
    ///         Ok(ProjectConfig::default())
    ///     }
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let bootstrapper = Bootstrapper::new(MyProject)
    ///     .with_config_name("test")?
    ///     .boot()
    ///     .await?;
    /// let bootstrapped_project = bootstrapper.into_bootstrapped_project();
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_config_name(self, config_name: &str) -> cot::Result<Bootstrapper<WithConfig>> {
        let config = self.project.config(config_name)?;

        Ok(self.with_config(config))
    }

    /// Sets the configuration for the project.
    ///
    /// This is mainly useful in tests, where you want to override the default
    /// behavior of reading the configuration from a file.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    /// use cot::{Bootstrapper, Project};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {}
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let bootstrapper = Bootstrapper::new(MyProject)
    ///     .with_config(ProjectConfig::default())
    ///     .boot()
    ///     .await?;
    /// let bootstrapped_project = bootstrapper.into_bootstrapped_project();
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_config(self, config: ProjectConfig) -> Bootstrapper<WithConfig> {
        Bootstrapper {
            project: self.project,
            context: self.context.with_config(config),
            handler: self.handler,
            error_handler: self.error_handler,
        }
    }
}

fn read_config(config: &str) -> cot::Result<ProjectConfig> {
    trace!(config, "Reading project configuration");
    let result = match std::fs::read_to_string(config) {
        Ok(config_content) => Ok(config_content),
        Err(_err) => {
            // try to read the config from the `config` directory if it's not a file
            let path = PathBuf::from("config").join(config).with_extension("toml");
            trace!(
                config,
                path = %path.display(),
                "Failed to read config as a file; trying to read from the `config` directory"
            );

            std::fs::read_to_string(&path)
        }
    };

    let config_content = result.map_err(|err| {
        Error::from_kind(ErrorKind::LoadConfig {
            config: config.to_owned(),
            source: err,
        })
    })?;

    ProjectConfig::from_toml(&config_content)
}

impl Bootstrapper<WithConfig> {
    /// Builds the Cot project instance.
    ///
    /// This is the final step in the bootstrapping process. It initializes the
    /// project with the given configuration and returns a [`Bootstrapper`]
    /// instance that contains the project's context and handler.
    ///
    /// You shouldn't have to use this method directly most of the time. It's
    /// mainly useful for controlling the bootstrapping process in custom
    /// [`CliTask`](cli::CliTask)s.
    ///
    /// # Errors
    ///
    /// This method may return an error if it cannot initialize any of the
    /// project's components, such as the database.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    /// use cot::{Bootstrapper, Project};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {}
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let bootstrapper = Bootstrapper::new(MyProject)
    ///     .with_config(ProjectConfig::default())
    ///     .boot()
    ///     .await?;
    /// let bootstrapped_project = bootstrapper.into_bootstrapped_project();
    /// # Ok(())
    /// # }
    /// ```
    // Send not needed; Bootstrapper is run async in a single thread
    #[expect(clippy::future_not_send)]
    pub async fn boot(self) -> cot::Result<Bootstrapper<Initialized>> {
        self.with_apps().boot().await
    }

    /// Moves forward to the next phase of bootstrapping, the with-apps phase.
    ///
    /// See the [`BootstrapPhase`] and [`WithApps`] documentation for more
    /// details.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    /// use cot::project::{Bootstrapper, WithApps};
    /// use cot::{AppBuilder, Project};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {}
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let bootstrapper = Bootstrapper::new(MyProject)
    ///     .with_config(ProjectConfig::default())
    ///     .with_apps()
    ///     .boot()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_apps(self) -> Bootstrapper<WithApps> {
        let mut module_builder = AppBuilder::new();
        self.project
            .register_apps(&mut module_builder, &self.context);

        let router = Arc::new(Router::with_urls(module_builder.urls));

        let context = self.context.with_apps(module_builder.apps, router);

        Bootstrapper {
            project: self.project,
            context,
            handler: self.handler,
            error_handler: self.error_handler,
        }
    }
}

impl Bootstrapper<WithApps> {
    /// Builds the Cot project instance.
    ///
    /// This is the final step in the bootstrapping process. It initializes the
    /// project with the given configuration and returns a [`Bootstrapper`]
    /// instance that contains the project's context and handler.
    ///
    /// You shouldn't have to use this method directly most of the time. It's
    /// mainly useful for controlling the bootstrapping process in custom
    /// [`CliTask`](cli::CliTask)s.
    ///
    /// # Errors
    ///
    /// This method may return an error if it cannot initialize any of the
    /// project's components, such as the database.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    /// use cot::{Bootstrapper, Project};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {}
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let bootstrapper = Bootstrapper::new(MyProject)
    ///     .with_config(ProjectConfig::default())
    ///     .with_apps()
    ///     .boot()
    ///     .await?;
    /// let bootstrapped_project = bootstrapper.into_bootstrapped_project();
    /// # Ok(())
    /// # }
    /// ```
    // Send not needed; Bootstrapper is run async in a single thread
    #[expect(clippy::future_not_send)]
    pub async fn boot(self) -> cot::Result<Bootstrapper<Initialized>> {
        self.with_database().await?.boot().await
    }

    /// Moves forward to the next phase of bootstrapping, the with-database
    /// phase.
    ///
    /// See the [`BootstrapPhase`] and [`WithDatabase`] documentation for more
    /// details.
    ///
    /// # Errors
    ///
    /// This method may return an error if it cannot initialize the database.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    /// use cot::project::{Bootstrapper, WithApps};
    /// use cot::{AppBuilder, Project};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {}
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let bootstrapper = Bootstrapper::new(MyProject)
    ///     .with_config(ProjectConfig::default())
    ///     .with_apps()
    ///     .with_database()
    ///     .await?
    ///     .boot()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    // Send not needed; Bootstrapper is run async in a single thread
    #[expect(clippy::future_not_send)]
    pub async fn with_database(self) -> cot::Result<Bootstrapper<WithDatabase>> {
        #[cfg(feature = "db")]
        let database = Self::init_database(&self.context.config.database).await?;
        let context = self.context.with_database(
            #[cfg(feature = "db")]
            database,
        );

        Ok(Bootstrapper {
            project: self.project,
            context,
            handler: self.handler,
            error_handler: self.error_handler,
        })
    }

    #[cfg(feature = "db")]
    async fn init_database(config: &DatabaseConfig) -> cot::Result<Option<Arc<Database>>> {
        match &config.url {
            Some(url) => {
                let database = Database::new(url.as_str()).await?;
                Ok(Some(Arc::new(database)))
            }
            None => Ok(None),
        }
    }
}

impl Bootstrapper<WithDatabase> {
    /// Builds the Cot project instance.
    ///
    /// This is the final step in the bootstrapping process. It initializes the
    /// project with the given configuration and returns a [`Bootstrapper`]
    /// instance that contains the project's context and handler.
    ///
    /// You shouldn't have to use this method directly most of the time. It's
    /// mainly useful for controlling the bootstrapping process in custom
    /// [`CliTask`](cli::CliTask)s.
    ///
    /// # Errors
    ///
    /// This method may return an error if it cannot initialize any of the
    /// project's components, such as the database.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    /// use cot::{Bootstrapper, Project};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {}
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let bootstrapper = Bootstrapper::new(MyProject)
    ///     .with_config(ProjectConfig::default())
    ///     .boot()
    ///     .await?;
    /// let bootstrapped_project = bootstrapper.into_bootstrapped_project();
    /// # Ok(())
    /// # }
    /// ```
    // Function marked `async` to be consistent with the other `boot` methods
    // Send not needed; Bootstrapper is run async in a single thread
    #[expect(clippy::unused_async, clippy::future_not_send)]
    pub async fn boot(self) -> cot::Result<Bootstrapper<Initialized>> {
        let router_service = RouterService::new(Arc::clone(&self.context.router));
        let handler_builder = RootHandlerBuilder {
            handler: router_service,
            error_handler: self.project.server_error_handler(),
        };
        let handler = self.project.middlewares(handler_builder, &self.context);

        let auth_backend = self.project.auth_backend(&self.context);
        let context = self.context.with_auth(auth_backend);

        Ok(Bootstrapper {
            project: self.project,
            context,
            handler: handler.handler,
            error_handler: handler.error_handler,
        })
    }
}

impl Bootstrapper<Initialized> {
    /// Returns the context and handlers of the bootstrapper.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    /// use cot::project::Bootstrapper;
    /// use cot::{Project, ProjectContext};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {}
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let bootstrapper = Bootstrapper::new(MyProject)
    ///     .with_config(ProjectConfig::default())
    ///     .boot()
    ///     .await?;
    /// let bootstrapped_project = bootstrapper.into_bootstrapped_project();
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn into_bootstrapped_project(self) -> BootstrappedProject {
        BootstrappedProject {
            context: self.context,
            handler: self.handler,
            error_handler: self.error_handler,
        }
    }
}

/// A fully bootstrapped project with all components initialized and ready to
/// run.
///
/// This struct contains the final state of a project after the bootstrapping
/// process is complete. It includes the project context with all dependencies
/// initialized, as well as the request and error handlers that are ready to
/// process incoming requests.
///
/// # Examples
///
/// ```
/// use cot::config::ProjectConfig;
/// use cot::project::Bootstrapper;
/// use cot::{Project, ProjectContext};
///
/// struct MyProject;
/// impl Project for MyProject {}
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let bootstrapper = Bootstrapper::new(MyProject)
///     .with_config(ProjectConfig::default())
///     .boot()
///     .await?;
/// let bootstrapped_project = bootstrapper.into_bootstrapped_project();
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub struct BootstrappedProject {
    /// The fully initialized project context containing all dependencies and
    /// configuration.
    pub context: ProjectContext<Initialized>,
    /// The main request handler that processes incoming HTTP requests.
    pub handler: BoxedHandler,
    /// The error handler that processes errors that occur during request
    /// handling.
    pub error_handler: BoxedHandler,
}

mod sealed {
    pub trait Sealed {}
}

/// A trait that represents the different phases of the bootstrapper.
///
/// This trait is used to define the types for the different phases of the
/// bootstrapper. It's used to ensure that you can't access nonexistent
/// data until the bootstrapper has reached the corresponding phase.
///
/// # Order of phases
///
/// 1. [`Uninitialized`]
/// 2. [`WithConfig`]
/// 3. [`WithApps`]
/// 4. [`WithDatabase`]
/// 5. [`Initialized`]
///
/// # Sealed
///
/// This trait is sealed and can't be implemented outside the `cot`
/// crate.
///
/// # Examples
///
/// ```
/// use cot::project::{MiddlewareContext, RegisterAppsContext, RootHandler, RootHandlerBuilder};
/// use cot::{AppBuilder, Project};
///
/// struct MyProject;
/// impl Project for MyProject {
///     // `WithConfig` phase here
///     fn register_apps(&self, apps: &mut AppBuilder, context: &RegisterAppsContext) {
///         unimplemented!();
///     }
///
///     // `WithDatabase` phase here (which comes after `WithConfig`)
///     fn middlewares(
///         &self,
///         handler: RootHandlerBuilder,
///         context: &MiddlewareContext,
///     ) -> RootHandler {
///         unimplemented!()
///     }
/// }
/// ```
pub trait BootstrapPhase: sealed::Sealed {
    // Bootstrapper types
    /// The type of the request handler.
    type RequestHandler: Debug;
    /// The type of the error handler.
    type ErrorHandler: Debug;

    // App context types
    /// The type of the configuration.
    type Config: Debug;
    /// The type of the apps.
    type Apps;
    /// The type of the router.
    type Router: Debug;
    /// The type of the database.
    #[cfg(feature = "db")]
    type Database: Debug;
    /// The type of the auth backend.
    type AuthBackend;
}

/// First phase of bootstrapping a Cot project, the uninitialized phase.
///
/// # See also
///
/// See the details about the different bootstrap phases in the
/// [`BootstrapPhase`] trait documentation.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Uninitialized {}

impl sealed::Sealed for Uninitialized {}
impl BootstrapPhase for Uninitialized {
    type RequestHandler = ();
    type ErrorHandler = ();
    type Config = ();
    type Apps = ();
    type Router = ();
    #[cfg(feature = "db")]
    type Database = ();
    type AuthBackend = ();
}

/// Second phase of bootstrapping a Cot project, the with-config phase.
///
/// # See also
///
/// See the details about the different bootstrap phases in the
/// [`BootstrapPhase`] trait documentation.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum WithConfig {}

impl sealed::Sealed for WithConfig {}
impl BootstrapPhase for WithConfig {
    type RequestHandler = ();
    type ErrorHandler = ();
    type Config = Arc<ProjectConfig>;
    type Apps = ();
    type Router = ();
    #[cfg(feature = "db")]
    type Database = ();
    type AuthBackend = ();
}

/// Third phase of bootstrapping a Cot project, the with-apps phase.
///
/// # See also
///
/// See the details about the different bootstrap phases in the
/// [`BootstrapPhase`] trait documentation.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum WithApps {}

impl sealed::Sealed for WithApps {}
impl BootstrapPhase for WithApps {
    type RequestHandler = ();
    type ErrorHandler = ();
    type Config = <WithConfig as BootstrapPhase>::Config;
    type Apps = Vec<Box<dyn App>>;
    type Router = Arc<Router>;
    #[cfg(feature = "db")]
    type Database = ();
    type AuthBackend = ();
}

/// Fourth phase of bootstrapping a Cot project, the with-database phase.
///
/// # See also
///
/// See the details about the different bootstrap phases in the
/// [`BootstrapPhase`] trait documentation.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum WithDatabase {}

impl sealed::Sealed for WithDatabase {}
impl BootstrapPhase for WithDatabase {
    type RequestHandler = ();
    type ErrorHandler = ();
    type Config = <WithApps as BootstrapPhase>::Config;
    type Apps = <WithApps as BootstrapPhase>::Apps;
    type Router = <WithApps as BootstrapPhase>::Router;
    #[cfg(feature = "db")]
    type Database = Option<Arc<Database>>;
    type AuthBackend = <WithApps as BootstrapPhase>::AuthBackend;
}

/// The final phase of bootstrapping a Cot project, the initialized phase.
///
/// # See also
///
/// See the details about the different bootstrap phases in the
/// [`BootstrapPhase`] trait documentation.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Initialized {}

impl sealed::Sealed for Initialized {}
impl BootstrapPhase for Initialized {
    type RequestHandler = BoxedHandler;
    type ErrorHandler = BoxedHandler;
    type Config = <WithDatabase as BootstrapPhase>::Config;
    type Apps = <WithDatabase as BootstrapPhase>::Apps;
    type Router = <WithDatabase as BootstrapPhase>::Router;
    #[cfg(feature = "db")]
    type Database = <WithDatabase as BootstrapPhase>::Database;
    type AuthBackend = Arc<dyn AuthBackend>;
}

/// Shared context and configs for all apps. Used in conjunction with the
/// [`Project`] trait.
#[derive(Debug)]
pub struct ProjectContext<S: BootstrapPhase = Initialized> {
    config: S::Config,
    #[debug("..")]
    apps: S::Apps,
    router: S::Router,
    #[cfg(feature = "db")]
    database: S::Database,
    #[debug("..")]
    auth_backend: S::AuthBackend,
}

impl ProjectContext<Uninitialized> {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            config: (),
            apps: (),
            router: (),
            #[cfg(feature = "db")]
            database: (),
            auth_backend: (),
        }
    }

    fn with_config(self, config: ProjectConfig) -> ProjectContext<WithConfig> {
        ProjectContext {
            config: Arc::new(config),
            apps: self.apps,
            router: self.router,
            #[cfg(feature = "db")]
            database: self.database,
            auth_backend: self.auth_backend,
        }
    }
}

impl<S: BootstrapPhase<Config = Arc<ProjectConfig>>> ProjectContext<S> {
    /// Returns the configuration for the project.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn index(request: Request) -> cot::Result<Response> {
    ///     let config = request.context().config();
    ///     // can also be accessed via:
    ///     let config = request.project_config();
    ///
    ///     let db_url = &config.database.url;
    ///
    ///     // ...
    /// #    unimplemented!()
    /// }
    /// ```
    #[must_use]
    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }
}

impl ProjectContext<WithConfig> {
    #[must_use]
    fn with_apps(self, apps: Vec<Box<dyn App>>, router: Arc<Router>) -> ProjectContext<WithApps> {
        ProjectContext {
            config: self.config,
            apps,
            router,
            #[cfg(feature = "db")]
            database: self.database,
            auth_backend: self.auth_backend,
        }
    }
}

impl<S: BootstrapPhase<Apps = Vec<Box<dyn App>>>> ProjectContext<S> {
    /// Returns the apps for the project.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn index(request: Request) -> cot::Result<Response> {
    ///     let apps = request.context().apps();
    ///
    ///     // ...
    /// #    unimplemented!()
    /// }
    /// ```
    #[must_use]
    pub fn apps(&self) -> &[Box<dyn App>] {
        &self.apps
    }
}

impl ProjectContext<WithApps> {
    #[must_use]
    fn with_database(
        self,
        #[cfg(feature = "db")] database: Option<Arc<Database>>,
    ) -> ProjectContext<WithDatabase> {
        ProjectContext {
            config: self.config,
            apps: self.apps,
            router: self.router,
            #[cfg(feature = "db")]
            database,
            auth_backend: self.auth_backend,
        }
    }
}

impl ProjectContext<WithDatabase> {
    #[must_use]
    fn with_auth(self, auth_backend: Arc<dyn AuthBackend>) -> ProjectContext<Initialized> {
        ProjectContext {
            config: self.config,
            apps: self.apps,
            router: self.router,
            auth_backend,
            #[cfg(feature = "db")]
            database: self.database,
        }
    }
}

impl ProjectContext<Initialized> {
    #[cfg(feature = "test")]
    pub(crate) fn initialized(
        config: <Initialized as BootstrapPhase>::Config,
        apps: <Initialized as BootstrapPhase>::Apps,
        router: <Initialized as BootstrapPhase>::Router,
        auth_backend: <Initialized as BootstrapPhase>::AuthBackend,
        #[cfg(feature = "db")] database: <Initialized as BootstrapPhase>::Database,
    ) -> Self {
        Self {
            config,
            apps,
            router,
            #[cfg(feature = "db")]
            database,
            auth_backend,
        }
    }
}

impl<S: BootstrapPhase<Router = Arc<Router>>> ProjectContext<S> {
    /// Returns the router for the project.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn index(request: Request) -> cot::Result<Response> {
    ///     let router = request.context().config();
    ///     // can also be accessed via:
    ///     let router = request.router();
    ///
    ///     let num_routes = router.routes().len();
    ///
    ///     // ...
    /// #    unimplemented!()
    /// }
    /// ```
    #[must_use]
    pub fn router(&self) -> &Arc<Router> {
        &self.router
    }
}
impl<S: BootstrapPhase<AuthBackend = Arc<dyn AuthBackend>>> ProjectContext<S> {
    /// Returns the authentication backend for the project.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn index(request: Request) -> cot::Result<Response> {
    ///     let auth_backend = request.context().auth_backend();
    ///     // ...
    /// #    unimplemented!()
    /// }
    /// ```
    #[must_use]
    pub fn auth_backend(&self) -> &Arc<dyn AuthBackend> {
        &self.auth_backend
    }
}

#[cfg(feature = "db")]
impl<S: BootstrapPhase<Database = Option<Arc<Database>>>> ProjectContext<S> {
    /// Returns the database for the project, if it is enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn index(request: Request) -> cot::Result<Response> {
    ///     let database = request.context().try_database();
    ///     if let Some(database) = database {
    ///         // do something with the database
    ///     } else {
    ///         // database is not enabled
    ///     }
    /// #    unimplemented!()
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
    ///
    /// async fn index(request: Request) -> cot::Result<Response> {
    ///     let database = request.context().database();
    ///     // can also be accessed via:
    ///     request.db();
    ///
    ///     // ...
    /// #    unimplemented!()
    /// }
    /// ```
    #[cfg(feature = "db")]
    #[must_use]
    #[track_caller]
    pub fn database(&self) -> &Arc<Database> {
        self.try_database().expect(
            "Database missing. Did you forget to add the database when configuring CotProject?",
        )
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
#[expect(
    clippy::future_not_send,
    reason = "Send not needed; Bootstrapper/CLI is run async in a single thread"
)]
pub async fn run(bootstrapper: Bootstrapper<Initialized>, address_str: &str) -> cot::Result<()> {
    let listener = tokio::net::TcpListener::bind(address_str)
        .await
        .map_err(|e| ErrorKind::StartServer { source: e })?;

    run_at(bootstrapper, listener).await
}

/// Runs the Cot project on the given listener.
///
/// This function takes a Cot project and a [`tokio::net::TcpListener`] and
/// runs the project on the given listener.
///
/// If you need more control over the server listening socket, such as modifying
/// the underlying buffer sizes, you can create a [`tokio::net::TcpListener`]
/// and pass it to this function. Otherwise, the [`run`] function will be more
/// convenient.
///
/// # Errors
///
/// This function returns an error if the server fails to start.
#[expect(
    clippy::future_not_send,
    reason = "Send not needed; Bootstrapper/CLI is run async in a single thread"
)]
pub async fn run_at(
    bootstrapper: Bootstrapper<Initialized>,
    listener: tokio::net::TcpListener,
) -> cot::Result<()> {
    run_at_with_shutdown(bootstrapper, listener, shutdown_signal()).await
}

/// Runs the Cot project on the given listener.
///
/// This function takes a Cot project and a [`tokio::net::TcpListener`] and
/// runs the project on the given listener, similarly to the [`run_at`]
/// function. In addition to that, it takes a shutdown signal that can be used
/// to gracefully shut down the server in a response to a signal or other event.
///
/// If you don't need to customize shutdown signal handling, you should instead
/// use the [`run`] or [`run_at`] functions, as they are more convenient.
///
/// # Errors
///
/// This function returns an error if the server fails to start.
#[expect(
    clippy::future_not_send,
    reason = "Send not needed; Bootstrapper/CLI is run async in a single thread"
)]
pub async fn run_at_with_shutdown(
    bootstrapper: Bootstrapper<Initialized>,
    listener: tokio::net::TcpListener,
    shutdown_signal: impl Future<Output = ()> + Send + 'static,
) -> cot::Result<()> {
    let BootstrappedProject {
        mut context,
        mut handler,
        mut error_handler,
    } = bootstrapper.into_bootstrapped_project();

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
    let is_debug = context.config().debug;
    let register_panic_hook = context.config().register_panic_hook;
    #[cfg(feature = "db")]
    let context_cleanup = context.clone();

    let handler = move |axum_request: axum::extract::Request| async move {
        // todo root tracing span
        // todo per-router error handlers
        let request = request_axum_to_cot(axum_request, Arc::clone(&context));
        let (request_parts, request) = request_parts_for_diagnostics(request);

        let catch_unwind_response = AssertUnwindSafe(pass_to_axum(request, &mut handler))
            .catch_unwind()
            .await;

        let response: Result<axum::response::Response, ErrorResponse> = match catch_unwind_response
        {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(error)) => Err(ErrorResponse::ErrorReturned(error)),
            Err(error) => Err(ErrorResponse::Panic(error)),
        };

        match response {
            Ok(response) => response,
            Err(error_response) => {
                if is_debug && accepts_html(&request_parts) {
                    let diagnostics = Diagnostics::new(
                        context.config().clone(),
                        Arc::clone(&context.router),
                        request_parts,
                    );

                    build_cot_error_page(error_response, &diagnostics)
                } else {
                    build_custom_error_page(
                        &mut error_handler,
                        error_response,
                        Arc::clone(&context),
                    )
                    .await
                }
            }
        }
    };

    eprintln!(
        "Starting the server at http://{}",
        listener
            .local_addr()
            .map_err(|e| ErrorKind::StartServer { source: e })?
    );

    if register_panic_hook {
        let current_hook = std::panic::take_hook();
        let new_hook = move |hook_info: &std::panic::PanicHookInfo<'_>| {
            current_hook(hook_info);
            error_page::error_page_panic_hook(hook_info);
        };
        std::panic::set_hook(Box::new(new_hook));
    }
    axum::serve(listener, handler.into_make_service())
        .with_graceful_shutdown(shutdown_signal)
        .await
        .map_err(|e| ErrorKind::StartServer { source: e })?;
    if register_panic_hook {
        let _ = std::panic::take_hook();
    }
    #[cfg(feature = "db")]
    if let Some(database) = &context_cleanup.database {
        database.close().await?;
    }

    Ok(())
}

fn accepts_html(parts: &Option<Parts>) -> bool {
    // todo parse the mime types???
    parts
        .as_ref()
        .and_then(|p| p.headers.get(http::header::ACCEPT))
        .map_or(false, |allow| {
            allow
                .to_str()
                .unwrap_or_default()
                .contains(HTML_NO_CHARSET_CONTENT_TYPE)
        })
}

enum ErrorResponse {
    ErrorReturned(Error),
    Panic(Box<dyn std::any::Any + Send>),
}

fn build_cot_error_page(
    error_response: ErrorResponse,
    diagnostics: &Diagnostics,
) -> axum::response::Response {
    match error_response {
        ErrorResponse::ErrorReturned(error) => {
            error_page::handle_response_error(&error, diagnostics)
        }
        ErrorResponse::Panic(error) => error_page::handle_response_panic(&error, diagnostics),
    }
}

async fn build_custom_error_page(
    server_error_handler: &mut BoxCloneSyncService<Request, Response, Error>,
    error_response: ErrorResponse,
    context: Arc<ProjectContext>,
) -> axum::response::Response {
    let error = match error_response {
        ErrorResponse::ErrorReturned(error) => error,
        ErrorResponse::Panic(payload) => Error::from(UncaughtPanic::new(payload)),
    };

    let request = build_request_for_error_handler(context, error);

    let poll_status = poll_fn(|cx| server_error_handler.poll_ready(cx)).await;
    if let Err(error) = poll_status {
        error!(
            ?error,
            "Error occurred when polling the server error handler for readiness; \
            returning default error page"
        );

        return error_page::build_cot_server_error_page();
    }

    let response = server_error_handler.call(request).await;
    response.map_or_else(
        |error| {
            error!(
                ?error,
                "Error occurred while running custom server error handler; \
                returning default error page"
            );

            error_page::build_cot_server_error_page()
        },
        response_cot_to_axum,
    )
}

#[must_use]
pub(crate) fn build_request_for_error_handler(
    context: Arc<ProjectContext>,
    error: Error,
) -> Request {
    let mut request = Request::default();
    prepare_request(&mut request, context);
    request.extensions_mut().insert(RequestError::new(error));
    request
}

/// Runs the CLI for the given project.
///
/// This function takes a [`Project`] and runs the CLI for the project. You
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
/// use cot::{App, Project, run_cli};
///
/// struct MyProject;
/// impl Project for MyProject {}
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// run_cli(MyProject).await?;
/// # Ok(())
/// # }
/// ```
#[expect(clippy::future_not_send)] // Send not needed; CLI is run async in a single thread
pub async fn run_cli(project: impl Project + 'static) -> cot::Result<()> {
    Bootstrapper::new(project).run_cli().await
}

fn request_parts_for_diagnostics(request: Request) -> (Option<Parts>, Request) {
    if request.project_config().debug {
        let (parts, body) = request.into_parts();
        let parts_clone = parts.clone();
        let request = Request::from_parts(parts, body);
        (Some(parts_clone), request)
    } else {
        (None, request)
    }
}

fn request_axum_to_cot(
    axum_request: axum::extract::Request,
    context: Arc<ProjectContext>,
) -> Request {
    let mut request = axum_request.map(Body::axum);
    prepare_request(&mut request, context);
    request
}

pub(crate) fn prepare_request(request: &mut Request, context: Arc<ProjectContext>) {
    request.extensions_mut().insert(context);
}

async fn pass_to_axum(
    request: Request,
    handler: &mut BoxedHandler,
) -> cot::Result<axum::response::Response> {
    poll_fn(|cx| handler.poll_ready(cx)).await?;
    let response = handler.call(request).await?;

    Ok(response_cot_to_axum(response))
}

fn response_cot_to_axum(response: Response) -> axum::response::Response {
    response.map(axum::body::Body::new)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StatusCode;
    use crate::auth::UserId;
    use crate::config::SecretKey;
    use crate::test::serial_guard;

    struct TestApp;

    impl App for TestApp {
        fn name(&self) -> &'static str {
            "mock"
        }
    }

    #[cot::test]
    async fn app_default_impl() {
        let app = TestApp {};
        assert_eq!(app.name(), "mock");
        assert_eq!(app.router().routes().len(), 0);
        assert_eq!(app.migrations().len(), 0);
    }

    struct TestProject;
    impl Project for TestProject {}

    #[test]
    fn project_default_cli_metadata() {
        let metadata = TestProject.cli_metadata();

        assert_eq!(metadata.name, "cot");
        assert_eq!(metadata.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(metadata.authors, env!("CARGO_PKG_AUTHORS"));
        assert_eq!(metadata.description, env!("CARGO_PKG_DESCRIPTION"));
    }

    #[cfg(feature = "live-reload")]
    #[cot::test]
    async fn project_middlewares() {
        struct TestProject;
        impl Project for TestProject {
            fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
                Ok(ProjectConfig::default())
            }

            fn middlewares(
                &self,
                handler: RootHandlerBuilder,
                context: &MiddlewareContext,
            ) -> RootHandler {
                handler
                    .middleware(crate::static_files::StaticFilesMiddleware::from_context(
                        context,
                    ))
                    .middleware(crate::middleware::LiveReloadMiddleware::from_context(
                        context,
                    ))
                    .build()
            }
        }

        let response = crate::test::Client::new(TestProject)
            .await
            .get("/")
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn project_default_config() {
        let temp_dir = tempfile::tempdir().unwrap();

        let config_dir = temp_dir.path().join("config");
        std::fs::create_dir(&config_dir).unwrap();
        let config = r#"
            debug = false
            secret_key = "123abc"
        "#;

        let config_file_path = config_dir.as_path().join("dev.toml");
        std::fs::write(config_file_path, config).unwrap();

        // ensure the tests run sequentially when setting the current directory
        let _guard = serial_guard();

        std::env::set_current_dir(&temp_dir).unwrap();
        let config = TestProject.config("dev").unwrap();

        assert!(!config.debug);
        assert_eq!(config.secret_key, SecretKey::from("123abc".to_string()));
    }

    #[test]
    fn project_default_register_apps() {
        let mut apps = AppBuilder::new();
        let context = ProjectContext::new().with_config(ProjectConfig::default());

        TestProject.register_apps(&mut apps, &context);

        assert!(apps.apps.is_empty());
    }

    #[cot::test]
    async fn test_default_auth_backend() {
        let context = ProjectContext::new()
            .with_config(
                ProjectConfig::builder()
                    .auth_backend(AuthBackendConfig::None)
                    .build(),
            )
            .with_apps(vec![], Arc::new(Router::empty()))
            .with_database(None);

        let auth_backend = TestProject.auth_backend(&context);
        assert!(
            auth_backend
                .get_by_id(UserId::Int(0))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[cot::test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
    )]
    async fn bootstrapper() {
        struct TestProject;
        impl Project for TestProject {
            fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
                apps.register_with_views(TestApp {}, "/app");
            }
        }

        let bootstrapper = Bootstrapper::new(TestProject)
            .with_config(ProjectConfig::default())
            .boot()
            .await
            .unwrap();

        assert_eq!(bootstrapper.context().apps.len(), 1);
        assert_eq!(bootstrapper.context().router.routes().len(), 1);
    }

    #[cot::test]
    async fn test_build_custom_error_page_poll_ready_failure() {
        use tower::service_fn;

        // Create a mock error handler that always fails on poll_ready
        let mock_handler = service_fn(|_request: Request| async {
            Err::<Response, Error>(Error::internal("Handler error"))
        });

        let mut error_handler = BoxCloneSyncService::new(mock_handler);

        // Create a test context
        let context = ProjectContext::new()
            .with_config(ProjectConfig::default())
            .with_apps(vec![], Arc::new(Router::empty()))
            .with_database(None)
            .with_auth(Arc::new(NoAuthBackend));

        // Create a panic error response
        let panic_payload = Box::new("Test panic message".to_string());
        let error_response = ErrorResponse::Panic(panic_payload);

        // Call build_custom_error_page - it should return the last-resort error page
        let response =
            build_custom_error_page(&mut error_handler, error_response, Arc::new(context)).await;

        // Verify that the last-resort error page was returned
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // The response should contain the default 500 error page content
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_string = String::from_utf8_lossy(&body_bytes);

        // Check that it contains the expected content from the 500.html template
        assert!(body_string.contains("Server Error"));
        assert!(
            body_string.contains("Sorry, the page you are looking for is currently unavailable")
        );
    }

    #[cot::test]
    async fn test_build_custom_error_page_call_failure() {
        use tower::service_fn;

        // Create a mock error handler that succeeds on poll_ready but fails on call
        let mock_handler = service_fn(|_request: Request| async {
            Err::<Response, Error>(Error::internal("Handler call failed"))
        });

        let mut error_handler = BoxCloneSyncService::new(mock_handler);

        // Create a test context
        let context = ProjectContext::new()
            .with_config(ProjectConfig::default())
            .with_apps(vec![], Arc::new(Router::empty()))
            .with_database(None)
            .with_auth(Arc::new(NoAuthBackend));

        // Create a regular error response
        let error = Error::internal("Test error");
        let error_response = ErrorResponse::ErrorReturned(error);

        // Call build_custom_error_page - it should return the last-resort error page
        let response =
            build_custom_error_page(&mut error_handler, error_response, Arc::new(context)).await;

        // Verify that the last-resort error page was returned
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // The response should contain the default 500 error page content
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_string = String::from_utf8_lossy(&body_bytes);

        // Check that it contains the expected content from the 500.html template
        assert!(body_string.contains("Server Error"));
        assert!(
            body_string.contains("Sorry, the page you are looking for is currently unavailable")
        );
    }

    #[cot::test]
    async fn test_build_custom_error_page_success() {
        use tower::service_fn;

        use crate::html::Html;

        // Create a mock error handler that succeeds
        let mock_handler = service_fn(|_request: Request| async {
            let html = Html::new("Custom error page content");
            Ok::<Response, Error>(html.into_response().unwrap())
        });

        let mut error_handler = BoxCloneSyncService::new(mock_handler);

        // Create a test context
        let context = ProjectContext::new()
            .with_config(ProjectConfig::default())
            .with_apps(vec![], Arc::new(Router::empty()))
            .with_database(None)
            .with_auth(Arc::new(NoAuthBackend));

        // Create a regular error response
        let error = Error::internal("Test error");
        let error_response = ErrorResponse::ErrorReturned(error);

        // Call build_custom_error_page - it should return the custom error page
        let response =
            build_custom_error_page(&mut error_handler, error_response, Arc::new(context)).await;

        // Verify that the custom error page was returned successfully
        assert_eq!(response.status(), StatusCode::OK);

        // The response should contain the custom content
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_string = String::from_utf8_lossy(&body_bytes);

        // Check that it contains the custom content
        assert!(body_string.contains("Custom error page content"));
    }

    #[cot::test]
    async fn test_build_custom_error_page_panic_conversion() {
        use tower::service_fn;

        use crate::error::handler::FromErrorRequestHead;
        use crate::html::Html;

        // Create a mock error handler that succeeds and verifies the error type
        let mock_handler = service_fn(|request: Request| async {
            // Extract the error from the request to verify it's an UncaughtPanic
            let (mut parts, _body) = request.into_parts();
            let error = Error::from_request_parts(&mut parts).await.unwrap();
            match error.kind() {
                ErrorKind::UncaughtPanic(_) => {}
                _ => panic!("Expected UncaughtPanic error kind"),
            }

            let html = Html::new("Panic error handled");
            Ok::<Response, Error>(html.into_response().unwrap())
        });

        let mut error_handler = BoxCloneSyncService::new(mock_handler);

        // Create a test context
        let context = ProjectContext::new()
            .with_config(ProjectConfig::default())
            .with_apps(vec![], Arc::new(Router::empty()))
            .with_database(None)
            .with_auth(Arc::new(NoAuthBackend));

        // Create a panic error response
        let panic_payload = Box::new("Test panic message".to_string());
        let error_response = ErrorResponse::Panic(panic_payload);

        // Call build_custom_error_page - it should convert panic to UncaughtPanic error
        let response =
            build_custom_error_page(&mut error_handler, error_response, Arc::new(context)).await;

        // Verify that the custom error page was returned successfully
        assert_eq!(response.status(), StatusCode::OK);

        // The response should contain the custom content
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_string = String::from_utf8_lossy(&body_bytes);

        // Check that it contains the custom content
        assert!(body_string.contains("Panic error handled"));
    }

    #[cot::test]
    async fn test_build_request_for_error_handler() {
        use crate::error::handler::RequestError;

        // Create a test context
        let context = ProjectContext::new()
            .with_config(ProjectConfig::default())
            .with_apps(vec![], Arc::new(Router::empty()))
            .with_database(None)
            .with_auth(Arc::new(NoAuthBackend));

        let error = Error::internal("Test error");

        // Call build_request_for_error_handler
        let request = build_request_for_error_handler(Arc::new(context), error);

        // Verify that the request has the project context
        assert!(request.extensions().get::<Arc<ProjectContext>>().is_some());

        // Verify that the request has the error in extensions
        let _request_error = request.extensions().get::<RequestError>().unwrap();
        // We can't directly access the error field, but we can verify it exists
        assert!(std::any::TypeId::of::<RequestError>() == std::any::TypeId::of::<RequestError>());
    }
}
