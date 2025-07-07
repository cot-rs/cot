use cot::middleware::{IntoCotErrorLayer, IntoCotResponseLayer};
use cot::project::MiddlewareContext;

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

#[cfg(test)]
mod tests {
    use crate::config::{LiveReloadMiddlewareConfig, MiddlewareConfig, ProjectConfig};
    use crate::{Bootstrapper, Project};

    #[cot::test]
    async fn live_reload_from_context_enabled() {
        test_live_reload_from_context(true).await;
    }

    #[cot::test]
    async fn live_reload_from_context_disabled() {
        test_live_reload_from_context(false).await;
    }

    #[expect(clippy::future_not_send, reason = "test function using Bootstrapper")]
    async fn test_live_reload_from_context(enabled: bool) {
        struct TestProject;
        impl Project for TestProject {}

        let middleware_config = LiveReloadMiddlewareConfig::builder()
            .enabled(enabled)
            .build();
        let config = ProjectConfig::builder()
            .middlewares(
                MiddlewareConfig::builder()
                    .live_reload(middleware_config)
                    .build(),
            )
            .build();
        let bootstrapper = Bootstrapper::new(TestProject)
            .with_config(config)
            .with_apps()
            .with_database()
            .await
            .unwrap();

        let middleware = super::LiveReloadMiddleware::from_context(bootstrapper.context());
        match middleware.0 {
            tower::util::Either::Left(_) => {
                assert!(enabled, "LiveReloadLayer should be disabled");
            }
            tower::util::Either::Right(_) => {
                assert!(!enabled, "LiveReloadLayer should be enabled");
            }
        }
    }
}
