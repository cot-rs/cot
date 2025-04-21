use std::borrow::Cow;
use std::convert::Infallible;

use bytes::{Bytes, BytesMut};
use cot::response::Response;
use cot::{Body, StatusCode};
use http;

/// Trait for generating responses.
/// Types that implement `IntoResponse` can be returned from handlers.
///
/// # Implementing `IntoResponse`
///
/// You generally shouldn't have to implement `IntoResponse` manually, as cot
/// provides implementations for many common types.
///
/// However, it might be necessary if you have a custom error type that you want
/// to return from handlers.

pub trait IntoResponse {
    /// Create a response.
    #[must_use]
    fn into_response(self) -> Response;

    fn with_header<K, V>(self, key: K, value: V) -> Response
    where
        K: Into<http::HeaderName>,
        V: Into<http::HeaderValue>,
        Self: Sized,
    {
        let key = key.into();
        let value = value.into();

        let mut response = self.into_response();
        response.headers_mut().append(key, value);
        response
    }

    fn with_content_type<V>(self, content_type: V) -> Response
    where
        V: Into<http::HeaderValue>,
        Self: Sized,
    {
        let mut response = self.into_response();
        response
            .headers_mut()
            .insert(http::header::CONTENT_TYPE, content_type.into());
        response
    }

    fn with_status(self, status: StatusCode) -> Response
    where
        Self: Sized,
    {
        let mut response = self.into_response();
        *response.status_mut() = status;
        response
    }

    fn with_body(self, body: impl Into<Body>) -> Response
    where
        Self: Sized,
    {
        let mut response = self.into_response();
        *response.body_mut() = body.into();
        response
    }
}
macro_rules! impl_into_response_for_type_and_mime {
    ($ty:ty, $mime:expr) => {
        impl IntoResponse for $ty {
            fn into_response(self) -> Response {
                let mut res = Body::from(self).into_response();
                res.headers_mut().insert(
                    http::header::CONTENT_TYPE,
                    http::HeaderValue::from_static($mime.as_ref()),
                );
                res
            }
        }
    };
}

// General implementations

impl IntoResponse for () {
    fn into_response(self) -> Response {
        Body::empty().into_response()
    }
}

impl IntoResponse for Infallible {
    fn into_response(self) -> Response {
        match self {}
    }
}

impl<R, E> IntoResponse for Result<R, E>
where
    R: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            Ok(value) => value.into_response(),
            Err(err) => err.into_response(),
        }
    }
}

impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}
// Text implementations
impl_into_response_for_type_and_mime!(&'static str, mime::TEXT_PLAIN_UTF_8);
impl_into_response_for_type_and_mime!(String, mime::TEXT_PLAIN_UTF_8);

impl IntoResponse for Box<str> {
    fn into_response(self) -> Response {
        String::from(self).into_response()
    }
}

// Bytes implementations
impl_into_response_for_type_and_mime!(&'static [u8], mime::APPLICATION_OCTET_STREAM);
impl_into_response_for_type_and_mime!(Vec<u8>, mime::APPLICATION_OCTET_STREAM);
impl_into_response_for_type_and_mime!(Bytes, mime::APPLICATION_OCTET_STREAM);

impl<const N: usize> IntoResponse for &'static [u8; N] {
    fn into_response(self) -> Response {
        self.as_slice().into_response()
    }
}

impl<const N: usize> IntoResponse for [u8; N] {
    fn into_response(self) -> Response {
        self.to_vec().into_response()
    }
}

impl IntoResponse for Box<[u8]> {
    fn into_response(self) -> Response {
        Vec::from(self).into_response()
    }
}

impl IntoResponse for BytesMut {
    fn into_response(self) -> Response {
        self.freeze().into_response()
    }
}

// HTTP structures for common uses

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.status_mut() = self;
        res
    }
}

impl IntoResponse for http::HeaderMap {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.headers_mut() = self;
        res
    }
}

impl IntoResponse for http::Extensions {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.extensions_mut() = self;
        res
    }
}

impl IntoResponse for http::response::Parts {
    fn into_response(self) -> Response {
        Response::from_parts(self, Body::empty())
    }
}

// Shortcuts for common uses

impl IntoResponse for Body {
    fn into_response(self) -> Response {
        Response::new(self)
    }
}

impl<R> IntoResponse for (StatusCode, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl<R> IntoResponse for (StatusCode, http::HeaderMap, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.2.into_response();
        *res.status_mut() = self.0;
        *res.headers_mut() = self.1;
        res
    }
}

impl<R> IntoResponse for (http::HeaderMap, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.1.into_response();
        *res.headers_mut() = self.0;
        res
    }
}
