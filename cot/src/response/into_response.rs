use std::borrow::Cow;
use std::convert::Infallible;

use bytes::{Bytes, BytesMut};
use cot::headers::HTML_CONTENT_TYPE;
use cot::response::{RESPONSE_BUILD_FAILURE, Response};
use cot::{Body, StatusCode};
use http;

use crate::html::Html;

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
    fn into_response(self) -> cot::Result<Response>;

    fn with_header<K, V>(self, key: K, value: V) -> cot::Result<Response>
    where
        K: Into<http::HeaderName>,
        V: Into<http::HeaderValue>,
        Self: Sized,
    {
        let key = key.into();
        let value = value.into();

        self.into_response().map(|mut resp| {
            resp.headers_mut().append(key, value);
            resp
        })
    }

    fn with_content_type<V>(self, content_type: V) -> cot::Result<Response>
    where
        V: Into<http::HeaderValue>,
        Self: Sized,
    {
        self.into_response().map(|mut resp| {
            resp.headers_mut()
                .insert(http::header::CONTENT_TYPE, content_type.into());
            resp
        })
    }

    fn with_status(self, status: StatusCode) -> cot::Result<Response>
    where
        Self: Sized,
    {
        self.into_response().map(|mut resp| {
            *resp.status_mut() = status;
            resp
        })
    }

    fn with_body(self, body: impl Into<Body>) -> cot::Result<Response>
    where
        Self: Sized,
    {
        self.into_response().map(|mut resp| {
            *resp.body_mut() = body.into();
            resp
        })
    }
}
macro_rules! impl_into_response_for_type_and_mime {
    ($ty:ty, $mime:expr) => {
        impl IntoResponse for $ty {
            fn into_response(self) -> cot::Result<Response> {
                Body::from(self).with_header(
                    http::header::CONTENT_TYPE,
                    http::HeaderValue::from_static($mime.as_ref()),
                )
            }
        }
    };
}

// General implementations

impl IntoResponse for () {
    fn into_response(self) -> cot::Result<Response> {
        Body::empty().into_response()
    }
}

impl IntoResponse for Infallible {
    fn into_response(self) -> cot::Result<Response> {
        match self {}
    }
}

impl<R, E> IntoResponse for Result<R, E>
where
    R: IntoResponse,
    E: Into<cot::Error>,
{
    fn into_response(self) -> cot::Result<Response> {
        match self {
            Ok(value) => value.into_response(),
            Err(err) => Err(err.into()),
        }
    }
}

impl IntoResponse for Response {
    fn into_response(self) -> cot::Result<Response> {
        Ok(self)
    }
}

// Text implementations

impl_into_response_for_type_and_mime!(&'static str, mime::TEXT_PLAIN_UTF_8);
impl_into_response_for_type_and_mime!(String, mime::TEXT_PLAIN_UTF_8);

impl IntoResponse for Box<str> {
    fn into_response(self) -> cot::Result<Response> {
        String::from(self).into_response()
    }
}

// Bytes implementations

impl_into_response_for_type_and_mime!(&'static [u8], mime::APPLICATION_OCTET_STREAM);
impl_into_response_for_type_and_mime!(Vec<u8>, mime::APPLICATION_OCTET_STREAM);
impl_into_response_for_type_and_mime!(Bytes, mime::APPLICATION_OCTET_STREAM);

impl<const N: usize> IntoResponse for &'static [u8; N] {
    fn into_response(self) -> cot::Result<Response> {
        self.as_slice().into_response()
    }
}

impl<const N: usize> IntoResponse for [u8; N] {
    fn into_response(self) -> cot::Result<Response> {
        self.to_vec().into_response()
    }
}

impl IntoResponse for Box<[u8]> {
    fn into_response(self) -> cot::Result<Response> {
        Vec::from(self).into_response()
    }
}

impl IntoResponse for BytesMut {
    fn into_response(self) -> cot::Result<Response> {
        self.freeze().into_response()
    }
}

// HTTP structures for common uses

impl IntoResponse for StatusCode {
    fn into_response(self) -> cot::Result<Response> {
        ().into_response().with_status(self)
    }
}

impl IntoResponse for http::HeaderMap {
    fn into_response(self) -> cot::Result<Response> {
        ().into_response().map(|mut resp| {
            *resp.headers_mut() = self;
            resp
        })
    }
}

impl IntoResponse for http::Extensions {
    fn into_response(self) -> cot::Result<Response> {
        ().into_response().map(|mut resp| {
            *resp.extensions_mut() = self;
            resp
        })
    }
}

impl IntoResponse for http::response::Parts {
    fn into_response(self) -> cot::Result<Response> {
        Ok(Response::from_parts(self, Body::empty()))
    }
}

// Data type structures implementations

impl IntoResponse for Html {
    /// Create a new HTML response.
    ///
    /// This creates a new [`Response`] object with a content type of
    /// `text/html; charset=utf-8` and given status code and body.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::Html;
    /// use cot::response::IntoResponse;
    ///
    /// let html = Html::new("<div>Hello</div>");
    ///
    /// let response = html.into_response();
    /// ```
    fn into_response(self) -> cot::Result<Response> {
        self.as_str().to_owned().into_response().with_content_type(
            http::header::HeaderValue::from_static(mime::TEXT_HTML_UTF_8.as_ref()),
        )
    }
}

// Shortcuts for common uses

impl IntoResponse for Body {
    fn into_response(self) -> cot::Result<Response> {
        Ok(Response::new(self))
    }
}
