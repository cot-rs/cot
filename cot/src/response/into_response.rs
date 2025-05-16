use bytes::{Bytes, BytesMut};
use cot::headers::{HTML_CONTENT_TYPE, OCTET_STREAM_CONTENT_TYPE, PLAIN_TEXT_CONTENT_TYPE};
use cot::response::Response;
use cot::{Body, Error, StatusCode};
use http;

use crate::error::ErrorRepr;
#[cfg(feature = "json")]
use crate::headers::JSON_CONTENT_TYPE;
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
    /// Converts the implementing type into a `cot::Result<Response>`.
    ///
    /// # Errors
    /// Returns an error if the conversion fails.
    fn into_response(self) -> cot::Result<Response>;

    /// Modifies the response by appending the specified header.
    ///
    /// # Errors
    /// Returns an error if the header name or value is invalid.
    fn with_header<K, V>(self, key: K, value: V) -> WithHeader<Self>
    where
        K: TryInto<http::HeaderName>,
        V: TryInto<http::HeaderValue>,
        Self: Sized,
    {
        let key = key.try_into().ok();
        let value = value.try_into().ok();

        WithHeader {
            inner: self,
            header: key.zip(value),
        }
    }

    /// Modifies the response by setting the `Content-Type` header.
    ///
    /// # Errors
    /// Returns an error if the content type value is invalid.
    fn with_content_type<V>(self, content_type: V) -> WithContentType<Self>
    where
        V: TryInto<http::HeaderValue>,
        Self: Sized,
    {
        WithContentType {
            inner: self,
            content_type: content_type.try_into().ok(),
        }
    }

    /// Modifies the response by setting the status code.
    ///
    /// # Errors
    /// Returns an error if the `IntoResponse` conversion fails.
    fn with_status(self, status: StatusCode) -> WithStatus<Self>
    where
        Self: Sized,
    {
        WithStatus {
            inner: self,
            status,
        }
    }

    /// Modifies the response by setting the body.
    ///
    /// # Errors
    /// Returns an error if the `IntoResponse` conversion fails.
    fn with_body(self, body: impl Into<Body>) -> WithBody<Self>
    where
        Self: Sized,
    {
        WithBody {
            inner: self,
            body: body.into(),
        }
    }

    /// Modifies the response by inserting an extension.
    ///
    /// # Errors
    /// Returns an error if the `IntoResponse` conversion fails.
    fn with_extension<T>(self, extension: T) -> WithExtension<Self, T>
    where
        T: Clone + Send + Sync + 'static,
        Self: Sized,
    {
        WithExtension {
            inner: self,
            extension,
        }
    }
}

/// Returned by [`with_header`](IntoResponse::with_header) method.
#[derive(Debug)]
pub struct WithHeader<T> {
    inner: T,
    header: Option<(http::HeaderName, http::HeaderValue)>,
}

impl<T: IntoResponse> IntoResponse for WithHeader<T> {
    fn into_response(self) -> cot::Result<Response> {
        self.inner.into_response().map(|mut resp| {
            if let Some((key, value)) = self.header {
                resp.headers_mut().append(key, value);
            }
            resp
        })
    }
}

/// Returned by [`with_content_type`](IntoResponse::with_content_type) method.
#[derive(Debug)]
pub struct WithContentType<T> {
    inner: T,
    content_type: Option<http::HeaderValue>,
}

impl<T: IntoResponse> IntoResponse for WithContentType<T> {
    fn into_response(self) -> cot::Result<Response> {
        self.inner.into_response().map(|mut resp| {
            if let Some(content_type) = self.content_type {
                resp.headers_mut()
                    .insert(http::header::CONTENT_TYPE, content_type);
            }
            resp
        })
    }
}

/// Returned by [`with_status`](IntoResponse::with_status) method.
#[derive(Debug)]
pub struct WithStatus<T> {
    inner: T,
    status: StatusCode,
}

impl<T: IntoResponse> IntoResponse for WithStatus<T> {
    fn into_response(self) -> cot::Result<Response> {
        self.inner.into_response().map(|mut resp| {
            *resp.status_mut() = self.status;
            resp
        })
    }
}

/// Returned by [`with_body`](IntoResponse::with_body) method.
#[derive(Debug)]
pub struct WithBody<T> {
    inner: T,
    body: Body,
}

impl<T: IntoResponse> IntoResponse for WithBody<T> {
    fn into_response(self) -> cot::Result<Response> {
        self.inner.into_response().map(|mut resp| {
            *resp.body_mut() = self.body;
            resp
        })
    }
}

/// Returned by [`with_extension`](IntoResponse::with_extension) method.
#[derive(Debug)]
pub struct WithExtension<T, D> {
    inner: T,
    extension: D,
}

impl<T, D> IntoResponse for WithExtension<T, D>
where
    T: IntoResponse,
    D: Clone + Send + Sync + 'static,
{
    fn into_response(self) -> cot::Result<Response> {
        self.inner.into_response().map(|mut resp| {
            resp.extensions_mut().insert(self.extension);
            resp
        })
    }
}

macro_rules! impl_into_response_for_type_and_mime {
    ($ty:ty, $mime:expr) => {
        impl IntoResponse for $ty {
            fn into_response(self) -> cot::Result<Response> {
                Body::from(self)
                    .with_header(http::header::CONTENT_TYPE, $mime)
                    .into_response()
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

impl<R, E> IntoResponse for Result<R, E>
where
    R: IntoResponse,
    E: Into<Error>,
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

impl_into_response_for_type_and_mime!(&'static str, PLAIN_TEXT_CONTENT_TYPE);
impl_into_response_for_type_and_mime!(String, PLAIN_TEXT_CONTENT_TYPE);

impl IntoResponse for Box<str> {
    fn into_response(self) -> cot::Result<Response> {
        String::from(self).into_response()
    }
}

// Bytes implementations

impl_into_response_for_type_and_mime!(&'static [u8], OCTET_STREAM_CONTENT_TYPE);
impl_into_response_for_type_and_mime!(Vec<u8>, OCTET_STREAM_CONTENT_TYPE);
impl_into_response_for_type_and_mime!(Bytes, OCTET_STREAM_CONTENT_TYPE);

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
        ().into_response().with_status(self).into_response()
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
    /// `text/html; charset=utf-8` and given body.
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
        self.0
            .into_response()
            .with_content_type(HTML_CONTENT_TYPE)
            .into_response()
    }
}

#[cfg(feature = "json")]
impl<D: serde::Serialize> IntoResponse for cot::json::Json<D> {
    /// Create a new JSON response.
    ///
    /// This creates a new [`Response`] object with a content type of
    /// `application/json` and given body.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// use cot::json::Json;
    /// use cot::response::IntoResponse;
    ///
    /// let data = HashMap::from([("hello", "world")]);
    /// let json = Json(data);
    ///
    /// let response = json.into_response();
    /// ```
    fn into_response(self) -> cot::Result<Response> {
        // a "reasonable default" for a JSON response size
        const DEFAULT_JSON_SIZE: usize = 128;

        let mut buf = Vec::with_capacity(DEFAULT_JSON_SIZE);
        let mut serializer = serde_json::Serializer::new(&mut buf);
        serde_path_to_error::serialize(&self.0, &mut serializer)
            .map_err(|error| Error::new(ErrorRepr::Json(error)))?;
        let data = String::from_utf8(buf).expect("JSON serialization always returns valid UTF-8");

        data.with_content_type(JSON_CONTENT_TYPE).into_response()
    }
}

// Shortcuts for common uses

impl IntoResponse for Body {
    fn into_response(self) -> cot::Result<Response> {
        Ok(Response::new(self))
    }
}

#[cfg(test)]
mod tests {
    use bytes::{Bytes, BytesMut};
    use cot::response::Response;
    use cot::{Body, StatusCode};
    use http::{self, HeaderMap, HeaderValue};

    use super::*;
    use crate::error::ErrorRepr;
    use crate::html::Html;

    #[tokio::test]
    async fn test_unit_into_response() {
        let response = ().into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().is_empty());
        assert_eq!(response.into_body().into_bytes().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_result_ok_into_response() {
        let res: Result<&'static str, Error> = Ok("hello");

        let response = res.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        assert_eq!(response.into_body().into_bytes().await.unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_result_err_into_response() {
        let err = Error::new(ErrorRepr::NotFound {
            message: Some("test".to_string()),
        });
        let res: Result<&'static str, Error> = Err(err);

        let error_result = res.into_response();

        assert!(error_result.is_err());
        assert!(error_result.err().unwrap().to_string().contains("test"));
    }

    #[tokio::test]
    async fn test_response_into_response() {
        let original_response = Response::new(Body::from("test"));

        let response = original_response.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.into_body().into_bytes().await.unwrap(), "test");
    }

    #[tokio::test]
    async fn test_static_str_into_response() {
        let response = "hello world".into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            "hello world"
        );
    }

    #[tokio::test]
    async fn test_string_into_response() {
        let s = String::from("hello string");

        let response = s.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            "hello string"
        );
    }

    #[tokio::test]
    async fn test_box_str_into_response() {
        let b: Box<str> = "hello box".into();

        let response = b.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            "hello box"
        );
    }

    #[tokio::test]
    async fn test_static_u8_slice_into_response() {
        let data: &'static [u8] = b"hello bytes";

        let response = data.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "application/octet-stream"
        );
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            "hello bytes"
        );
    }

    #[tokio::test]
    async fn test_vec_u8_into_response() {
        let data: Vec<u8> = vec![1, 2, 3];

        let response = data.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "application/octet-stream"
        );
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            Bytes::from(vec![1, 2, 3])
        );
    }

    #[tokio::test]
    async fn test_bytes_into_response() {
        let data = Bytes::from_static(b"hello bytes obj");

        let response = data.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "application/octet-stream"
        );
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            "hello bytes obj"
        );
    }

    #[tokio::test]
    async fn test_static_u8_array_into_response() {
        let data: &'static [u8; 5] = b"array";

        let response = data.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "application/octet-stream"
        );
        assert_eq!(response.into_body().into_bytes().await.unwrap(), "array");
    }

    #[tokio::test]
    async fn test_u8_array_into_response() {
        let data: [u8; 3] = [4, 5, 6];

        let response = data.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "application/octet-stream"
        );
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            Bytes::from(vec![4, 5, 6])
        );
    }

    #[tokio::test]
    async fn test_box_u8_slice_into_response() {
        let data: Box<[u8]> = Box::new([7, 8, 9]);

        let response = data.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "application/octet-stream"
        );
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            Bytes::from(vec![7, 8, 9])
        );
    }

    #[tokio::test]
    async fn test_bytes_mut_into_response() {
        let mut data = BytesMut::with_capacity(10);
        data.extend_from_slice(b"mutable");

        let response = data.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "application/octet-stream"
        );
        assert_eq!(response.into_body().into_bytes().await.unwrap(), "mutable");
    }

    #[tokio::test]
    async fn test_status_code_into_response() {
        let response = StatusCode::NOT_FOUND.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(response.headers().is_empty());
        assert_eq!(response.into_body().into_bytes().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_header_map_into_response() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Test", HeaderValue::from_static("value"));

        let response = headers.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("X-Test").unwrap(), "value");
        assert_eq!(response.into_body().into_bytes().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_extensions_into_response() {
        let mut extensions = http::Extensions::new();
        extensions.insert("My Extension Data");

        let response = extensions.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().is_empty());
        assert_eq!(
            response.extensions().get::<&str>(),
            Some(&"My Extension Data")
        );
        assert_eq!(response.into_body().into_bytes().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_parts_into_response() {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::ACCEPTED;
        response
            .headers_mut()
            .insert("X-From-Parts", HeaderValue::from_static("yes"));
        response.extensions_mut().insert(123usize);
        let (parts, _) = response.into_parts();

        let new_response = parts.into_response().unwrap();

        assert_eq!(new_response.status(), StatusCode::ACCEPTED);
        assert_eq!(new_response.headers().get("X-From-Parts").unwrap(), "yes");
        assert_eq!(new_response.extensions().get::<usize>(), Some(&123));
        assert_eq!(
            new_response.into_body().into_bytes().await.unwrap().len(),
            0
        );
    }

    #[tokio::test]
    async fn test_html_into_response() {
        let html = Html::new("<h1>Test</h1>");

        let response = html.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            "<h1>Test</h1>"
        );
    }

    #[tokio::test]
    async fn test_body_into_response() {
        let body = Body::from("body test");

        let response = body.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE),
            None // Body itself doesn't set content-type
        );
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            "body test"
        );
    }

    #[tokio::test]
    async fn test_with_header() {
        let response = "test"
            .with_header("X-Custom", "HeaderValue")
            .into_response()
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("X-Custom").unwrap(), "HeaderValue");
        assert_eq!(response.into_body().into_bytes().await.unwrap(), "test");
    }

    #[tokio::test]
    async fn test_with_content_type() {
        let response = "test"
            .with_content_type("application/json")
            .into_response()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
        assert_eq!(response.into_body().into_bytes().await.unwrap(), "test");
    }

    #[tokio::test]
    async fn test_with_status() {
        let response = "test"
            .with_status(StatusCode::CREATED)
            .into_response()
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        assert_eq!(response.into_body().into_bytes().await.unwrap(), "test");
    }

    #[tokio::test]
    async fn test_with_body() {
        let response = StatusCode::ACCEPTED
            .with_body("new body")
            .into_response()
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.into_body().into_bytes().await.unwrap(), "new body");
    }

    #[tokio::test]
    async fn test_with_extension() {
        #[derive(Clone, Debug, PartialEq)]
        struct MyExt(String);

        let response = "test"
            .with_extension(MyExt("data".to_string()))
            .into_response()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.extensions().get::<MyExt>(),
            Some(&MyExt("data".to_string()))
        );
        assert_eq!(response.into_body().into_bytes().await.unwrap(), "test");
    }

    #[cfg(feature = "json")]
    #[tokio::test]
    async fn test_json_struct_into_response() {
        use serde::Serialize;

        #[derive(Serialize, PartialEq, Debug)]
        struct TestData {
            name: String,
            value: i32,
        }

        let data = TestData {
            name: "test".to_string(),
            value: 123,
        };
        let json = cot::json::Json(data);
        let response = json.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            JSON_CONTENT_TYPE
        );

        let body_bytes = response.into_body().into_bytes().await.unwrap();
        let expected_json = r#"{"name":"test","value":123}"#;

        assert_eq!(body_bytes, expected_json.as_bytes());
    }

    #[cfg(feature = "json")]
    #[tokio::test]
    async fn test_json_hashmap_into_response() {
        use std::collections::HashMap;

        let data = HashMap::from([("key", "value")]);
        let json = cot::json::Json(data);
        let response = json.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            JSON_CONTENT_TYPE
        );

        let body_bytes = response.into_body().into_bytes().await.unwrap();
        let expected_json = r#"{"key":"value"}"#;
        assert_eq!(body_bytes, expected_json.as_bytes());
    }
}
