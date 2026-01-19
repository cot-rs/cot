//! Middlewares for modifying requests and responses.
//!
//! Middlewares are used to modify requests and responses in a pipeline. They
//! are used to add functionality to the request/response cycle, such as
//! session management, adding security headers, and more.

use std::fmt::Debug;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_util::TryFutureExt;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use tower::Service;

use crate::request::Request;
use crate::response::Response;
use crate::{Body, Error};

#[derive(Debug, Copy, Clone)]
pub struct IntoCotResponseLayer;

impl IntoCotResponseLayer {
    /// Create a new [`IntoCotResponseLayer`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::IntoCotResponseLayer;
    ///
    /// let middleware = IntoCotResponseLayer::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for IntoCotResponseLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> tower::Layer<S> for IntoCotResponseLayer {
    type Service = IntoCotResponse<S>;

    fn layer(&self, inner: S) -> Self::Service {
        IntoCotResponse { inner }
    }
}

/// Service struct that converts any [`http::Response`] generic
/// type to [`Response`].
///
/// Used by [`IntoCotResponseLayer`].
///
/// # Examples
///
/// ```
/// use std::any::TypeId;
///
/// use cot::middleware::{IntoCotResponse, IntoCotResponseLayer};
///
/// assert_eq!(
///     TypeId::of::<<IntoCotResponseLayer as tower::Layer<()>>::Service>(),
///     TypeId::of::<IntoCotResponse::<()>>()
/// );
/// ```
#[derive(Debug, Clone)]
pub struct IntoCotResponse<S> {
    inner: S,
}

impl<S, ResBody, E> Service<Request> for IntoCotResponse<S>
where
    S: Service<Request, Response = http::Response<ResBody>>,
    ResBody: http_body::Body<Data = Bytes, Error = E> + Send + Sync + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = futures_util::future::MapOk<S::Future, fn(http::Response<ResBody>) -> Response>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, request: Request) -> Self::Future {
        self.inner.call(request).map_ok(map_response)
    }
}

fn map_response<ResBody, E>(response: http::response::Response<ResBody>) -> Response
where
    ResBody: http_body::Body<Data = Bytes, Error = E> + Send + Sync + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    response.map(|body| Body::wrapper(BoxBody::new(body.map_err(map_err))))
}

#[derive(Debug, Copy, Clone)]
pub struct IntoCotErrorLayer;

impl IntoCotErrorLayer {
    /// Create a new [`IntoCotErrorLayer`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::middleware::IntoCotErrorLayer;
    ///
    /// let middleware = IntoCotErrorLayer::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for IntoCotErrorLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> tower::Layer<S> for IntoCotErrorLayer {
    type Service = IntoCotError<S>;

    fn layer(&self, inner: S) -> Self::Service {
        IntoCotError { inner }
    }
}

/// Service struct that converts a any error type to a [`Error`].
///
/// Used by [`IntoCotErrorLayer`].
///
/// # Examples
///
/// ```
/// use std::any::TypeId;
///
/// use cot::middleware::{IntoCotError, IntoCotErrorLayer};
///
/// assert_eq!(
///     TypeId::of::<<IntoCotErrorLayer as tower::Layer<()>>::Service>(),
///     TypeId::of::<IntoCotError::<()>>()
/// );
/// ```
#[derive(Debug, Clone)]
pub struct IntoCotError<S> {
    inner: S,
}

impl<S> Service<Request> for IntoCotError<S>
where
    S: Service<Request>,
    <S as Service<Request>>::Error: std::error::Error + Send + Sync + 'static,
{
    type Response = S::Response;
    type Error = Error;
    type Future = futures_util::future::MapErr<S::Future, fn(S::Error) -> Error>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(map_err)
    }

    #[inline]
    fn call(&mut self, request: Request) -> Self::Future {
        self.inner.call(request).map_err(map_err)
    }
}

fn map_err<E>(error: E) -> Error
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[expect(trivial_casts)]
    let boxed = Box::new(error) as Box<dyn std::error::Error + Send + Sync>;
    boxed.downcast::<Error>().map_or_else(Error::wrap, |e| *e)
}
