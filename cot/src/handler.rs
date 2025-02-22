use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use cot::request::extractors::FromRequest;
use tower::util::BoxCloneSyncService;

use crate::error::ErrorRepr;
use crate::request::extractors::FromRequestParts;
use crate::request::Request;
use crate::response::{not_found_response, Response};
use crate::{Error, Result};

/// A function that takes a request and returns a response.
///
/// This is the main building block of a Cot app. You shouldn't
/// usually need to implement this directly, as it is already
/// implemented for closures and functions that take a [`Request`]
/// and return a [`Result<Response>`].
pub trait RequestHandler<T = ()> {
    /// Handle the request and returns a response.
    ///
    /// # Errors
    ///
    /// This method can return an error if the request handler fails to handle
    /// the request.
    fn handle(&self, request: Request) -> impl Future<Output = Result<Response>> + Send;
}

pub(crate) trait BoxRequestHandler {
    fn handle(
        &self,
        request: Request,
    ) -> Pin<Box<dyn Future<Output = Result<Response>> + Send + '_>>;
}

pub(crate) fn into_box_request_handler<T, H: RequestHandler<T> + Send + Sync>(
    handler: H,
) -> impl BoxRequestHandler {
    struct Inner<T, H>(H, PhantomData<fn() -> T>);

    impl<T, H: RequestHandler<T> + Send + Sync> BoxRequestHandler for Inner<T, H> {
        fn handle(
            &self,
            request: Request,
        ) -> Pin<Box<dyn Future<Output = Result<Response>> + Send + '_>> {
            Box::pin(async move {
                let response = self.0.handle(request).await;

                match response {
                    Ok(response) => Ok(response),
                    Err(error) => match error.inner {
                        ErrorRepr::NotFound { message } => Ok(not_found_response(message)),
                        _ => Err(error),
                    },
                }
            })
        }
    }

    Inner(handler, PhantomData)
}

impl<T, P1, R> RequestHandler<(P1,)> for T
where
    T: Fn(P1) -> R + Clone + Send + Sync + 'static,
    P1: FromRequest + Send,
    R: for<'a> Future<Output = Result<Response>> + Send,
{
    async fn handle(&self, request: Request) -> Result<Response> {
        let p1 = P1::from_request(request).await?;

        self(p1).await
    }
}

impl<T, P1, P2, R> RequestHandler<(P1, (), P2)> for T
where
    T: Fn(P1, P2) -> R + Clone + Send + Sync + 'static,
    P1: FromRequestParts + Send,
    P2: FromRequest + Send,
    R: for<'a> Future<Output = Result<Response>> + Send,
{
    async fn handle(&self, request: Request) -> Result<Response> {
        let (mut parts, body) = request.into_parts();
        let p1 = P1::from_request_parts(&mut parts).await?;

        let request = Request::from_parts(parts, body);
        let p2 = P2::from_request(request).await?;

        self(p1, p2).await
    }
}

impl<T, P1, P2, R> RequestHandler<(P2, P1, ())> for T
where
    T: Fn(P1, P2) -> R + Clone + Send + Sync + 'static,
    P1: FromRequest + Send,
    P2: FromRequestParts + Send,
    R: for<'a> Future<Output = Result<Response>> + Send,
{
    async fn handle(&self, request: Request) -> Result<Response> {
        let (mut parts, body) = request.into_parts();
        let p2 = P2::from_request_parts(&mut parts).await?;

        let request = Request::from_parts(parts, body);
        let p1 = P1::from_request(request).await?;

        self(p1, p2).await
    }
}

/// A wrapper around a handler that's used in
/// [`Bootstrapper`](cot::Bootstrapper).
///
/// It is returned by
/// [`Bootstrapper::into_context_and_handler`](cot::Bootstrapper::into_context_and_handler).
/// Typically, you don't need to interact with this type directly, except for
/// creating it in [`Project::middlewares`](cot::Project::middlewares) through
/// the [`RootHandlerBuilder::build`](cot::project::RootHandlerBuilder::build).
/// method.
///
/// # Examples
///
/// ```
/// use cot::config::ProjectConfig;
/// use cot::project::{RootHandlerBuilder, WithApps};
/// use cot::static_files::StaticFilesMiddleware;
/// use cot::{Bootstrapper, BoxedHandler, Project, ProjectContext};
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn middlewares(
///         &self,
///         handler: RootHandlerBuilder,
///         context: &ProjectContext<WithApps>,
///     ) -> BoxedHandler {
///         handler
///             .middleware(StaticFilesMiddleware::from_context(context))
///             .build()
///     }
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let bootstrapper = Bootstrapper::new(MyProject)
///     .with_config(ProjectConfig::default())
///     .boot()
///     .await?;
/// let (context, handler) = bootstrapper.into_context_and_handler();
/// # Ok(())
/// # }
/// ```
pub type BoxedHandler = BoxCloneSyncService<Request, Response, Error>;
