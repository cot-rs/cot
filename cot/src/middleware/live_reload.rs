use std::convert::Infallible;
use std::task::{Context, Poll};

use cot::Error;
use cot::error::ErrorKind;
use cot::middleware::{IntoCotErrorLayer, IntoCotResponseLayer};
use cot::project::MiddlewareContext;
use cot::request::Request;
use cot::response::Response;
use cot::static_files::StaticFilesService;
use futures_util::TryFutureExt;
use tower::Service;

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
#[derive(Debug, Clone)]
pub struct LiveReloadMiddleware(LiveReloadLayerType);

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
        let option_layer = enabled.then(|| IntoCotErrorLayer2::new());
        Self(tower::util::option_layer(option_layer))
    }
}

impl Default for LiveReloadMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

type LiveReloadLayerType = tower::util::Either<IntoCotErrorLayer2, tower::layer::util::Identity>;

impl<S> tower::Layer<S> for LiveReloadMiddleware {
    type Service = <LiveReloadLayerType as tower::Layer<S>>::Service;

    fn layer(&self, inner: S) -> Self::Service {
        self.0.layer(inner)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct IntoCotErrorLayer2;

impl IntoCotErrorLayer2 {
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for IntoCotErrorLayer2 {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> tower::Layer<S> for IntoCotErrorLayer2 {
    type Service = IntoCotError2<S>;

    fn layer(&self, inner: S) -> Self::Service {
        IntoCotError2 {
            inner: tower_livereload::LiveReloadLayer::new().layer(inner),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntoCotError2<S> {
    inner: tower_livereload::LiveReload<S>,
}

impl<ReqBody, S> Service<http::Request<ReqBody>> for IntoCotError2<S>
where
    S: Service<http::Request<ReqBody>, Response = Response>,
{
    type Response = S::Response;
    type Error = Error;
    type Future = futures_util::future::MapErr<S::Future, fn(S::Error) -> Error>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(|error| {
            type X<S, B> = <tower_livereload::LiveReload<S> as Service<http::Request<B>>>::Error;

            match error {
                X::<S, ReqBody>::A(_) => {
                    todo!()
                }
                X::<S, ReqBody>::B(xd) => {
                    todo!()
                }
            }
        })
    }

    #[inline]
    fn call(&mut self, request: http::Request<ReqBody>) -> Self::Future {
        todo!()
        // self.inner.call(request).map_err(map_err)
    }
}

// fn map_err(error: ) -> Error {
// todo!()
// #[expect(trivial_casts)]
// let boxed = Box::new(error) as Box<dyn std::error::Error + Send + Sync>;
// boxed
//     .downcast::<Error>()
//     .map(|e| *e)
//     .unwrap_or_else(|boxed| Error::new(ErrorRepr::MiddlewareWrapped {
// source: boxed }))
// }
