//! Error handling functionality for custom error pages and handlers.

use std::marker::PhantomData;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use derive_more::with_trait::Debug;

use crate::Error;
use crate::handler::handle_all_parameters;
use crate::request::extractors::FromRequestHead;
use crate::request::{Request, RequestHead};
use crate::response::Response;

/// A trait for handling error pages in Cot applications.
///
/// This trait is implemented by functions that can handle error pages. The
/// trait is automatically implemented for async functions that take parameters
/// implementing [`FromRequestHead`] and return a type that implements
/// [`IntoResponse`].
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
/// // This function automatically implements ErrorPageHandler
/// async fn error_handler(error: Error) -> impl IntoResponse {
///     Html::new(format!("An error occurred: {error}")).with_status(error.status_code())
/// }
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid error page handler",
    label = "not a valid error page handler",
    note = "make sure the function is marked `async`",
    note = "make sure all parameters implement `FromErrorRequestParts`",
    note = "make sure the function takes no more than 10 parameters",
    note = "make sure the function returns a type that implements `IntoResponse`"
)]
pub trait ErrorPageHandler<T = ()> {
    /// Handles an error request and returns a response.
    ///
    /// This method is called when an error occurs and the application needs to
    /// generate an error page response.
    ///
    /// Note that the request passed to this method is **not** the original
    /// request that caused the error, but rather a new request that contains
    /// the error information in its extensions, along with the project context.
    /// This allows the handler to generate a response based on the error
    /// context without having to retain the original request.
    ///
    /// # Errors
    ///
    /// This method may return an error if the handler fails to build a
    /// response. In this case, the error will be logged and a generic
    /// error page will be returned to the user.
    fn handle(&self, head: &RequestHead) -> impl Future<Output = crate::Result<Response>> + Send;
}

pub(crate) trait BoxErrorPageHandler: Send + Sync {
    fn handle<'a>(
        &'a self,
        head: &'a RequestHead,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Response>> + Send + '_>>;
}

/// A type-erased wrapper around an error page handler.
///
/// This struct allows storing different types of error page handlers in a
/// homogeneous collection or service. It implements [`Clone`] and can be
/// used with Cot's error handling infrastructure.
#[derive(Debug, Clone)]
pub struct DynErrorPageHandler {
    #[debug("..")]
    handler: Arc<dyn BoxErrorPageHandler>,
}

impl DynErrorPageHandler {
    /// Creates a new `DynErrorPageHandler` from a concrete error page handler.
    ///
    /// This method wraps a concrete error page handler in a type-erased
    /// wrapper, allowing it to be used in
    /// [`crate::project::Project::server_error_handler`].
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
    /// // This function automatically implements ErrorPageHandler
    /// async fn error_handler(error: Error) -> impl IntoResponse {
    ///     Html::new(format!("An error occurred: {error}")).with_status(error.status_code())
    /// }
    /// ```
    pub fn new<HandlerParams, H>(handler: H) -> Self
    where
        HandlerParams: 'static,
        H: ErrorPageHandler<HandlerParams> + Send + Sync + 'static,
    {
        struct Inner<T, H>(H, PhantomData<fn() -> T>);

        impl<T, H: ErrorPageHandler<T> + Send + Sync> BoxErrorPageHandler for Inner<T, H> {
            fn handle<'a>(
                &'a self,
                head: &'a RequestHead,
            ) -> Pin<Box<dyn Future<Output = cot::Result<Response>> + Send + '_>> {
                Box::pin(self.0.handle(head))
            }
        }

        Self {
            handler: Arc::new(Inner(handler, PhantomData)),
        }
    }
}

impl tower::Service<Request> for DynErrorPageHandler {
    type Response = Response;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = cot::Result<Self::Response>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let handler = self.handler.clone();
        let (head, _) = req.into_parts();
        Box::pin(async move { handler.handle(&head).await })
    }
}

macro_rules! impl_request_handler {
    ($($ty:ident),*) => {
        impl<Func, $($ty,)* Fut, R> ErrorPageHandler<($($ty,)*)> for Func
        where
            Func: FnOnce($($ty,)*) -> Fut + Clone + Send + Sync + 'static,
            $($ty: FromRequestHead + Send,)*
            Fut: Future<Output = R> + Send,
            R: crate::response::IntoResponse,
        {
            #[allow(
                clippy::allow_attributes,
                non_snake_case,
                reason = "for the case where there are no params"
            )]
            async fn handle(&self, head: &RequestHead) -> crate::Result<Response> {
                #[allow(unused_variables, unused_mut)] // for the case where there are no params
                $(
                    let $ty = <$ty as FromRequestHead>::from_request_head(&head).await?;
                )*

                self.clone()($($ty,)*).await.into_response()
            }
        }
    };
}

handle_all_parameters!(impl_request_handler);

/// A simple wrapper around `crate::Error` that indicates that it is an error
/// returned by the request handler.
///
/// It is a separate, private type to make sure the user cannot accidentally
/// interact with it by using request extensions directly.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct RequestError(Arc<Error>);

impl RequestError {
    #[must_use]
    pub(crate) fn new(error: Error) -> Self {
        Self(Arc::new(error))
    }
}

impl Deref for RequestError {
    type Target = Error;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromRequestHead for RequestError {
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        let error = head.extensions.get::<RequestError>();
        error
            .ok_or_else(|| {
                Error::internal("No error found in request head. Make sure you use this extractor in an error handler.")
            })
            .map(|request_error| request_error.clone())
    }
}
