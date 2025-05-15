use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use tower::util::BoxCloneSyncService;

use crate::request::Request;
use crate::request::extractors::{FromRequest, FromRequestParts};
use crate::response::{IntoResponse, Response};
use crate::{Error, Result};

/// A function that takes a request and returns a response.
///
/// This is the main building block of a Cot app. You shouldn't
/// usually need to implement this directly, as it is already
/// implemented for closures and functions that take some
/// number of [extractors](crate::request::extractors) as parameters
/// and return some type that [can be converted into a
/// response](IntoResponse).
///
/// # Details
///
/// Cot provides an implementation of `RequestHandler` for functions
/// and closures that:
/// * are marked `async`
/// * take at most 10 parameters, all of which implement [`FromRequestParts`],
///   except for at most one that implements [`FromRequest`]
/// * return a type that implements [`IntoResponse`]
/// * is `Clone + Send + 'static` (important if it's a closure)
/// * return a future that is `Send` (i.e., doesn't hold any non-Send references
///   across await points)
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
            Box::pin(self.0.handle(request))
        }
    }

    Inner(handler, PhantomData)
}

macro_rules! impl_request_handler {
    ($($ty:ident),*) => {
        impl<Func, $($ty,)* Fut, R> RequestHandler<($($ty,)*)> for Func
        where
            Func: FnOnce($($ty,)*) -> Fut + Clone + Send + Sync + 'static,
            $($ty: FromRequestParts + Send,)*
            Fut: Future<Output = R> + Send,
            R: IntoResponse,
        {
            #[allow(non_snake_case)]
            async fn handle(&self, request: Request) -> Result<Response> {
                #[allow(unused_variables, unused_mut)] // for the case where there are no params
                let (mut parts, _body) = request.into_parts();

                $(
                    let $ty = $ty::from_request_parts(&mut parts).await?;
                )*

                self.clone()($($ty,)*).await.into_response()
            }
        }
    };
}

macro_rules! impl_request_handler_from_request {
    ($($ty_lhs:ident,)* ($ty_from_request:ident) $(,$ty_rhs:ident)*) => {
        impl<Func, $($ty_lhs,)* $ty_from_request, $($ty_rhs,)* Fut, R> RequestHandler<($($ty_lhs,)* $ty_from_request, (), $($ty_rhs,)*)> for Func
        where
            Func: FnOnce($($ty_lhs,)* $ty_from_request, $($ty_rhs),*) -> Fut + Clone + Send + Sync + 'static,
            $($ty_lhs: FromRequestParts + Send,)*
            $ty_from_request: FromRequest + Send,
            $($ty_rhs: FromRequestParts + Send,)*
            Fut: Future<Output = R> + Send,
            R: IntoResponse,
        {
            #[expect(non_snake_case)]
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

                self.clone()($($ty_lhs,)* $ty_from_request, $($ty_rhs),*).await.into_response()
            }
        }
    };
}

macro_rules! handle_all_parameters {
    ($name:ident) => {
        $name!();
        $name!(P1);
        $name!(P1, P2);
        $name!(P1, P2, P3);
        $name!(P1, P2, P3, P4);
        $name!(P1, P2, P3, P4, P5);
        $name!(P1, P2, P3, P4, P5, P6);
        $name!(P1, P2, P3, P4, P5, P6, P7);
        $name!(P1, P2, P3, P4, P5, P6, P7, P8);
        $name!(P1, P2, P3, P4, P5, P6, P7, P8, P9);
        $name!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10);
    };
}

macro_rules! handle_all_parameters_from_request {
    ($name:ident) => {
        $name!((PX));

        $name!((PX), P2);
        $name!(P1, (PX));

        $name!((PX), P2, P3);
        $name!(P1, (PX), P3);
        $name!(P1, P2, (PX));

        $name!((PX), P2, P3, P4);
        $name!(P1, (PX), P3, P4);
        $name!(P1, P2, (PX), P4);
        $name!(P1, P2, P3, (PX));

        $name!((PX), P2, P3, P4, P5);
        $name!(P1, (PX), P3, P4, P5);
        $name!(P1, P2, (PX), P4, P5);
        $name!(P1, P2, P3, (PX), P5);
        $name!(P1, P2, P3, P4, (PX));

        $name!((PX), P2, P3, P4, P5, P6);
        $name!(P1, (PX), P3, P4, P5, P6);
        $name!(P1, P2, (PX), P4, P5, P6);
        $name!(P1, P2, P3, (PX), P5, P6);
        $name!(P1, P2, P3, P4, (PX), P6);
        $name!(P1, P2, P3, P4, P5, (PX));

        $name!((PX), P2, P3, P4, P5, P6, P7);
        $name!(P1, (PX), P3, P4, P5, P6, P7);
        $name!(P1, P2, (PX), P4, P5, P6, P7);
        $name!(P1, P2, P3, (PX), P5, P6, P7);
        $name!(P1, P2, P3, P4, (PX), P6, P7);
        $name!(P1, P2, P3, P4, P5, (PX), P7);
        $name!(P1, P2, P3, P4, P5, P6, (PX));

        $name!((PX), P2, P3, P4, P5, P6, P7, P8);
        $name!(P1, (PX), P3, P4, P5, P6, P7, P8);
        $name!(P1, P2, (PX), P4, P5, P6, P7, P8);
        $name!(P1, P2, P3, (PX), P5, P6, P7, P8);
        $name!(P1, P2, P3, P4, (PX), P6, P7, P8);
        $name!(P1, P2, P3, P4, P5, (PX), P7, P8);
        $name!(P1, P2, P3, P4, P5, P6, (PX), P8);
        $name!(P1, P2, P3, P4, P5, P6, P7, (PX));

        $name!((PX), P2, P3, P4, P5, P6, P7, P8, P9);
        $name!(P1, (PX), P3, P4, P5, P6, P7, P8, P9);
        $name!(P1, P2, (PX), P4, P5, P6, P7, P8, P9);
        $name!(P1, P2, P3, (PX), P5, P6, P7, P8, P9);
        $name!(P1, P2, P3, P4, (PX), P6, P7, P8, P9);
        $name!(P1, P2, P3, P4, P5, (PX), P7, P8, P9);
        $name!(P1, P2, P3, P4, P5, P6, (PX), P8, P9);
        $name!(P1, P2, P3, P4, P5, P6, P7, (PX), P9);
        $name!(P1, P2, P3, P4, P5, P6, P7, P8, (PX));

        $name!((PX), P2, P3, P4, P5, P6, P7, P8, P9, P10);
        $name!(P1, (PX), P3, P4, P5, P6, P7, P8, P9, P10);
        $name!(P1, P2, (PX), P4, P5, P6, P7, P8, P9, P10);
        $name!(P1, P2, P3, (PX), P5, P6, P7, P8, P9, P10);
        $name!(P1, P2, P3, P4, (PX), P6, P7, P8, P9, P10);
        $name!(P1, P2, P3, P4, P5, (PX), P7, P8, P9, P10);
        $name!(P1, P2, P3, P4, P5, P6, (PX), P8, P9, P10);
        $name!(P1, P2, P3, P4, P5, P6, P7, (PX), P9, P10);
        $name!(P1, P2, P3, P4, P5, P6, P7, P8, (PX), P10);
        $name!(P1, P2, P3, P4, P5, P6, P7, P8, P9, (P10));
    };
}

handle_all_parameters!(impl_request_handler);
handle_all_parameters_from_request!(impl_request_handler_from_request);

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
