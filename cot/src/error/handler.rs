use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;

use cot::RequestHandler;
use cot::handler::BoxRequestHandler;
use cot::request::Request;
use cot::response::Response;
pub use cot_macros::FromErrorRequestParts;
use http::request::Parts;

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid error page handler",
    label = "not a valid error page handler",
    note = "make sure the function is marked `async`",
    note = "make sure all parameters implement `FromErrorRequestParts`",
    note = "make sure the function takes no more than 10 parameters"
)]
pub trait ErrorPageHandler<T = ()> {
    fn handle(&self, request: Request) -> impl Future<Output = crate::Result<Response>> + Send;
}

pub(crate) trait BoxErrorPageHandler: Send + Sync {
    fn handle(
        &self,
        request: Request,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Response>> + Send + '_>>;
}

pub struct DynErrorPageHandler {
    handler: Box<dyn BoxErrorPageHandler>,
}

impl DynErrorPageHandler {
    pub fn new<HandlerParams, H>(handler: H) -> Self
    where
        HandlerParams: 'static,
        H: ErrorPageHandler<HandlerParams> + Send + Sync + 'static,
    {
        struct Inner<T, H>(H, PhantomData<fn() -> T>);

        impl<T, H: ErrorPageHandler<T> + Send + Sync> BoxErrorPageHandler for Inner<T, H> {
            fn handle(
                &self,
                request: Request,
            ) -> Pin<Box<dyn Future<Output = cot::Result<Response>> + Send + '_>> {
                Box::pin(self.0.handle(request))
            }
        }

        Self {
            handler: Box::new(Inner(handler, PhantomData)),
        }
    }

    pub fn into_handler(self) -> Box<dyn BoxErrorPageHandler> {
        self.handler
    }
}

macro_rules! impl_request_handler {
    ($($ty:ident),*) => {
        impl<Func, $($ty,)* Fut, R> ErrorPageHandler<($($ty,)*)> for Func
        where
            Func: FnOnce($($ty,)*) -> Fut + Clone + Send + Sync + 'static,
            $($ty: FromErrorRequestParts + Send,)*
            Fut: Future<Output = R> + Send,
            R: crate::response::IntoResponse,
        {
            #[allow(non_snake_case)]
            async fn handle(&self, request: Request) -> crate::Result<Response> {
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

handle_all_parameters!(impl_request_handler);

pub trait FromErrorRequestParts: Sized {
    /// Extracts data from the request parts.
    ///
    /// # Errors
    ///
    /// Throws an error if the extractor fails to extract the data from the
    /// request parts.
    fn from_request_parts(parts: &mut Parts) -> impl Future<Output = crate::Result<Self>> + Send;
}

#[doc(hidden)]
#[macro_export]
macro_rules! impl_from_error_request_parts {
    ($ty:ty) => {
        impl $crate::error::handler::FromErrorRequestParts for $ty {
            fn from_request_parts(
                parts: &mut $crate::http::request::Parts,
            ) -> impl ::std::future::Future<Output = $crate::Result<Self>> + Send {
                <$ty as $crate::request::extractors::FromRequestParts>::from_request_parts(parts)
            }
        }
    };
}

pub use impl_from_error_request_parts;

use crate::handler::handle_all_parameters;

impl_from_error_request_parts!(crate::router::Urls);
impl_from_error_request_parts!(crate::request::extractors::StaticFiles);

/// A simple wrapper around `crate::Error` that indicates that it is an error
/// returned by the request handler.
///
/// It is a separate, private type to make sure the user cannot accidentally
/// interact with it by using request extensions directly.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub(crate) struct RequestError(Arc<crate::Error>);

impl RequestError {
    #[must_use]
    pub(crate) fn new(error: crate::Error) -> Self {
        Self(Arc::new(error))
    }
}

impl FromErrorRequestParts for crate::Error {
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        let error = parts.extensions.remove::<RequestError>();
        error
            .ok_or_else(|| {
                crate::Error::new("No error found in request parts. Was it extracted already?")
            })
            .map(|e| Arc::into_inner(e.0).expect("RequestError was cloned"))
    }
}
