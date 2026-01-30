//! Request handler traits and utilities.
//!
//! This module provides the [`RequestHandler`] trait, which is the core
//! abstraction for handling HTTP requests in Cot. It is automatically
//! implemented for async functions taking extractors and returning responses.

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use tower::util::BoxCloneSyncService;

use crate::request::Request;
use crate::request::extractors::{FromRequest, FromRequestHead};
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
/// * take at most 10 parameters, all of which implement [`FromRequestHead`],
///   except for at most one that implements [`FromRequest`]
/// * return a type that implements [`IntoResponse`]
/// * is `Clone + Send + 'static` (important if it's a closure)
/// * return a future that is `Send` (i.e., doesn't hold any non-Send references
///   across await points)
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid request handler",
    label = "not a valid request handler",
    note = "make sure the function is marked `async`",
    note = "make sure all parameters implement `FromRequest` or `FromRequestHead`",
    note = "make sure there is at most one parameter implementing `FromRequest`",
    note = "make sure the function takes no more than 10 parameters",
    note = "make sure the function returns a type that implements `IntoResponse`"
)]
pub trait RequestHandler<T = ()> {
    /// Handle the request and returns a response.
    ///
    /// # Errors
    ///
    /// This method can return an error if it fails to handle the request.
    fn handle(&self, request: Request) -> impl Future<Output = Result<Response>> + Send;
}

pub trait BoxRequestHandler {
    fn handle(
        &self,
        request: Request,
    ) -> Pin<Box<dyn Future<Output = Result<Response>> + Send + '_>>;
}

pub fn into_box_request_handler<T, H: RequestHandler<T> + Send + Sync>(
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
            $($ty: FromRequestHead + Send,)*
            Fut: Future<Output = R> + Send,
            R: IntoResponse,
        {
            #[allow(
                clippy::allow_attributes,
                non_snake_case,
                reason = "for the case where there are no params"
            )]
            async fn handle(&self, request: Request) -> Result<Response> {
                #[allow(
                    clippy::allow_attributes,
                    unused_variables,
                    reason = "for the case where there are no params"
                )]
                let (head, _body) = request.into_parts();

                $(
                    let $ty = <$ty as FromRequestHead>::from_request_head(&head).await?;
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
            $($ty_lhs: FromRequestHead + Send,)*
            $ty_from_request: FromRequest + Send,
            $($ty_rhs: FromRequestHead + Send,)*
            Fut: Future<Output = R> + Send,
            R: IntoResponse,
        {
            #[allow(
                clippy::allow_attributes,
                non_snake_case,
                reason = "for the case where there are no FromRequestHead params"
            )]
            async fn handle(&self, request: Request) -> Result<Response> {
                #[allow(
                    clippy::allow_attributes,
                    reason = "for the case where there are no FromRequestHead params"
                )]
                let (head, body) = request.into_parts();

                $(
                    let $ty_lhs = $ty_lhs::from_request_head(&head).await?;
                )*
                $(
                    let $ty_rhs = $ty_rhs::from_request_head(&head).await?;
                )*

                let $ty_from_request = $ty_from_request::from_request(&head, body).await?;

                self.clone()($($ty_lhs,)* $ty_from_request, $($ty_rhs),*).await.into_response()
            }
        }
    };
}

#[macro_export]
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

pub use handle_all_parameters;

handle_all_parameters!(impl_request_handler);
handle_all_parameters_from_request!(impl_request_handler_from_request);

pub type BoxedHandler = BoxCloneSyncService<Request, Response, Error>;
