//! Error handling functionality for custom error pages and handlers.

use std::fmt::Display;
use std::marker::PhantomData;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use cot_core::handler::handle_all_parameters;
use derive_more::with_trait::Debug;

use crate::Error;
use crate::request::extractors::FromRequestHead;
use crate::request::{Request, RequestHead};
use crate::response::Response;

/// A trait for handling error pages in Cot applications.
///
/// This trait is implemented by functions that can handle error pages. The
/// trait is automatically implemented for async functions that take parameters
/// implementing [`FromRequestHead`] and return a type that implements
/// [`IntoResponse`](crate::response::IntoResponse).
///
/// # Examples
///
/// ```
/// use cot::Project;
/// use cot::error::handler::{DynErrorPageHandler, RequestError};
/// use cot::html::Html;
/// use cot::response::IntoResponse;
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn error_handler(&self) -> DynErrorPageHandler {
///         DynErrorPageHandler::new(error_handler)
///     }
/// }
///
/// // This function automatically implements ErrorPageHandler
/// async fn error_handler(error: RequestError) -> impl IntoResponse {
///     Html::new(format!("An error occurred: {error}")).with_status(error.status_code())
/// }
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid error page handler",
    label = "not a valid error page handler",
    note = "make sure the function is marked `async`",
    note = "make sure all parameters implement `FromRequestHead`",
    note = "make sure the function takes no more than 10 parameters",
    note = "make sure the function returns a type that implements `IntoResponse`"
)]
pub trait ErrorPageHandler<T = ()> {
    /// Handles an error request and returns a response.
    ///
    /// This method is called when an error occurs and the application needs to
    /// generate an error page response.
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
    ) -> Pin<Box<dyn Future<Output = crate::Result<Response>> + Send + 'a>>;
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
    /// [`crate::project::Project::error_handler`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Project;
    /// use cot::error::handler::{DynErrorPageHandler, RequestError};
    /// use cot::html::Html;
    /// use cot::response::IntoResponse;
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn error_handler(&self) -> DynErrorPageHandler {
    ///         DynErrorPageHandler::new(error_handler)
    ///     }
    /// }
    ///
    /// // This function automatically implements ErrorPageHandler
    /// async fn error_handler(error: RequestError) -> impl IntoResponse {
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
            ) -> Pin<Box<dyn Future<Output = cot::Result<Response>> + Send + 'a>> {
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
                unused_variables,
                reason = "for the case where there are no params"
            )]
            async fn handle(&self, head: &RequestHead) -> crate::Result<Response> {
                $(
                    let $ty = <$ty as FromRequestHead>::from_request_head(&head).await?;
                )*

                self.clone()($($ty,)*).await.into_response()
            }
        }
    };
}

handle_all_parameters!(impl_request_handler);

/// A wrapper around [`Error`] that contains an error (outermost,
/// possibly middleware-wrapped) to be processed by the error handler.
///
/// This returns the outermost error returned by the request handler and
/// middlewares. In most cases, you should use [`RequestError`] instead, which
/// dereferences to the inner [`Error`] (the first error in the error chain that
/// contains an explicitly set status code). [`RequestError`] will allow you to
/// check for specific error types even when middleware might have wrapped the
/// error.
#[derive(Debug, Clone)]
pub struct RequestOuterError(Arc<Error>);

impl RequestOuterError {
    #[must_use]
    pub(crate) fn new(error: Error) -> Self {
        Self(Arc::new(error))
    }
}

impl Deref for RequestOuterError {
    type Target = Error;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for RequestOuterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0.inner(), f)
    }
}

impl FromRequestHead for RequestOuterError {
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        let error = head.extensions.get::<RequestOuterError>();
        error
            .ok_or_else(|| {
                Error::internal("No error found in request head. Make sure you use this extractor in an error handler.")
            }).cloned()
    }
}

/// A wrapper around [`Error`] that contains an error to be processed by the
/// error handler.
///
/// Note that the [`Deref`] implementation returns the inner [`Error`] (see
/// [`Error::inner`]), which is the first error in the error chain that contains
/// an explicitly set status code. This is usually what you want since it allows
/// you to check for specific error types even when middleware might have
/// wrapped the error. If you need to access the outermost error instead,
/// you can use [`RequestOuterError`].
#[derive(Debug, Clone)]
pub struct RequestError(Arc<Error>);

impl Deref for RequestError {
    type Target = Error;

    fn deref(&self) -> &Self::Target {
        self.0.inner()
    }
}

impl Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0.inner(), f)
    }
}

impl FromRequestHead for RequestError {
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        let error = head.extensions.get::<RequestOuterError>();
        error
            .ok_or_else(|| {
                Error::internal(
                    "No error found in request head. \
                Make sure you use this extractor in an error handler.",
                )
            })
            .map(|request_error| Self(request_error.0.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_outer_error_display() {
        let error = Error::internal("Test error");
        let request_error = RequestOuterError::new(error);

        assert_eq!(format!("{request_error}"), "Test error");
    }

    #[test]
    fn request_error_display() {
        let error = Error::internal("Test error");
        let request_error = RequestError(Arc::new(error));

        assert_eq!(format!("{request_error}"), "Test error");
    }

    #[cot::test]
    async fn request_outer_error_from_request_head() {
        let request = Request::default();
        let (mut head, _) = request.into_parts();
        head.extensions
            .insert(RequestOuterError::new(Error::internal("Test error")));

        let extracted_error = RequestOuterError::from_request_head(&head).await.unwrap();
        assert_eq!(format!("{extracted_error}"), "Test error");
    }

    #[cot::test]
    async fn request_error_from_request_head() {
        let request = Request::default();
        let (mut head, _) = request.into_parts();
        head.extensions
            .insert(RequestOuterError::new(Error::internal("Test error")));

        let extracted_error = RequestError::from_request_head(&head).await.unwrap();
        assert_eq!(format!("{extracted_error}"), "Test error");
    }
}
