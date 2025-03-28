use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use tower::util::BoxCloneSyncService;

use crate::error::ErrorRepr;
use crate::request::Request;
use crate::request::extractors::{FromRequest, FromRequestParts};
use crate::response::{Response, not_found_response};
use crate::{Error, Result};

/// A function that takes a request and returns a response.
///
/// This is the main building block of a Cot app. You shouldn't
/// usually need to implement this directly, as it is already
/// implemented for closures and functions that take a [`Request`]
/// and return a [`Result<Response>`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid request handler",
    label = "not a valid request handler",
    note = "make sure the function is marked `async`",
    note = "make sure all parameters implement `FromRequest` or `FromRequestParts`",
    note = "make sure there is at most one parameter implementing `FromRequest`",
    note = "make sure the function takes no more than 10 parameters"
)]
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

macro_rules! impl_request_handler {
    ($($ty:ident),*) => {
        impl<T, $($ty,)* R> RequestHandler<($($ty,)*)> for T
        where
            T: Fn($($ty,)*) -> R + Clone + Send + Sync + 'static,
            $($ty: FromRequestParts + Send,)*
            R: for<'a> Future<Output = Result<Response>> + Send,
        {
            #[allow(non_snake_case)]
            async fn handle(&self, request: Request) -> Result<Response> {
                #[allow(unused_variables, unused_mut)] // for the case where there are no params
                let (mut parts, _body) = request.into_parts();

                $(
                    let $ty = $ty::from_request_parts(&mut parts).await?;
                )*

                self($($ty,)*).await
            }
        }
    };
}

macro_rules! impl_request_handler_from_request {
    ($($ty_lhs:ident,)* ($ty_from_request:ident) $(,$ty_rhs:ident)*) => {
        impl<T, $($ty_lhs,)* $ty_from_request, $($ty_rhs,)* R> RequestHandler<($($ty_lhs,)* $ty_from_request, (), $($ty_rhs,)*)> for T
        where
            T: Fn($($ty_lhs,)* $ty_from_request, $($ty_rhs),*) -> R + Clone + Send + Sync + 'static,
            $($ty_lhs: FromRequestParts + Send,)*
            $ty_from_request: FromRequest + Send,
            $($ty_rhs: FromRequestParts + Send,)*
            R: for<'a> Future<Output = Result<Response>> + Send,
        {
            #[allow(non_snake_case)]
            async fn handle(&self, request: Request) -> Result<Response> {
                #[allow(unused_mut)] // for the case where there are no FromRequestParts params
                let (mut parts, body) = request.into_parts();

                $(
                    let $ty_lhs = $ty_lhs::from_request_parts(&mut parts).await?;
                )*
                $(
                    let $ty_rhs = $ty_rhs::from_request_parts(&mut parts).await?;
                )*

                let request = Request::from_parts(parts, body);
                let $ty_from_request = $ty_from_request::from_request(request).await?;

                self($($ty_lhs,)* $ty_from_request, $($ty_rhs),*).await
            }
        }
    };
}

impl_request_handler!();
impl_request_handler!(P1);
impl_request_handler!(P1, P2);
impl_request_handler!(P1, P2, P3);
impl_request_handler!(P1, P2, P3, P4);
impl_request_handler!(P1, P2, P3, P4, P5);
impl_request_handler!(P1, P2, P3, P4, P5, P6);
impl_request_handler!(P1, P2, P3, P4, P5, P6, P7);
impl_request_handler!(P1, P2, P3, P4, P5, P6, P7, P8);
impl_request_handler!(P1, P2, P3, P4, P5, P6, P7, P8, P9);
impl_request_handler!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10);

impl_request_handler_from_request!((P1));
impl_request_handler_from_request!((P1), P2);
impl_request_handler_from_request!(P1, (P2));
impl_request_handler_from_request!((P1), P2, P3);
impl_request_handler_from_request!(P1, (P2), P3);
impl_request_handler_from_request!(P1, P2, (P3));
impl_request_handler_from_request!((P1), P2, P3, P4);
impl_request_handler_from_request!(P1, (P2), P3, P4);
impl_request_handler_from_request!(P1, P2, (P3), P4);
impl_request_handler_from_request!(P1, P2, P3, (P4));
impl_request_handler_from_request!((P1), P2, P3, P4, P5);
impl_request_handler_from_request!(P1, (P2), P3, P4, P5);
impl_request_handler_from_request!(P1, P2, (P3), P4, P5);
impl_request_handler_from_request!(P1, P2, P3, (P4), P5);
impl_request_handler_from_request!(P1, P2, P3, P4, (P5));
impl_request_handler_from_request!((P1), P2, P3, P4, P5, P6);
impl_request_handler_from_request!(P1, (P2), P3, P4, P5, P6);
impl_request_handler_from_request!(P1, P2, (P3), P4, P5, P6);
impl_request_handler_from_request!(P1, P2, P3, (P4), P5, P6);
impl_request_handler_from_request!(P1, P2, P3, P4, (P5), P6);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, (P6));
impl_request_handler_from_request!((P1), P2, P3, P4, P5, P6, P7);
impl_request_handler_from_request!(P1, (P2), P3, P4, P5, P6, P7);
impl_request_handler_from_request!(P1, P2, (P3), P4, P5, P6, P7);
impl_request_handler_from_request!(P1, P2, P3, (P4), P5, P6, P7);
impl_request_handler_from_request!(P1, P2, P3, P4, (P5), P6, P7);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, (P6), P7);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, P6, (P7));
impl_request_handler_from_request!((P1), P2, P3, P4, P5, P6, P7, P8);
impl_request_handler_from_request!(P1, (P2), P3, P4, P5, P6, P7, P8);
impl_request_handler_from_request!(P1, P2, (P3), P4, P5, P6, P7, P8);
impl_request_handler_from_request!(P1, P2, P3, (P4), P5, P6, P7, P8);
impl_request_handler_from_request!(P1, P2, P3, P4, (P5), P6, P7, P8);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, (P6), P7, P8);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, P6, (P7), P8);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, P6, P7, (P8));
impl_request_handler_from_request!((P1), P2, P3, P4, P5, P6, P7, P8, P9);
impl_request_handler_from_request!(P1, (P2), P3, P4, P5, P6, P7, P8, P9);
impl_request_handler_from_request!(P1, P2, (P3), P4, P5, P6, P7, P8, P9);
impl_request_handler_from_request!(P1, P2, P3, (P4), P5, P6, P7, P8, P9);
impl_request_handler_from_request!(P1, P2, P3, P4, (P5), P6, P7, P8, P9);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, (P6), P7, P8, P9);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, P6, (P7), P8, P9);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, P6, P7, (P8), P9);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, P6, P7, P8, (P9));
impl_request_handler_from_request!((P1), P2, P3, P4, P5, P6, P7, P8, P9, P10);
impl_request_handler_from_request!(P1, (P2), P3, P4, P5, P6, P7, P8, P9, P10);
impl_request_handler_from_request!(P1, P2, (P3), P4, P5, P6, P7, P8, P9, P10);
impl_request_handler_from_request!(P1, P2, P3, (P4), P5, P6, P7, P8, P9, P10);
impl_request_handler_from_request!(P1, P2, P3, P4, (P5), P6, P7, P8, P9, P10);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, (P6), P7, P8, P9, P10);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, P6, (P7), P8, P9, P10);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, P6, P7, (P8), P9, P10);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, P6, P7, P8, (P9), P10);
impl_request_handler_from_request!(P1, P2, P3, P4, P5, P6, P7, P8, P9, (P10));

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
/// use cot::project::{MiddlewareContext, RootHandlerBuilder};
/// use cot::static_files::StaticFilesMiddleware;
/// use cot::{Bootstrapper, BoxedHandler, Project, ProjectContext};
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn middlewares(
///         &self,
///         handler: RootHandlerBuilder,
///         context: &MiddlewareContext,
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
