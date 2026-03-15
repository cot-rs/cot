//! Extractors for request data.
//!
//! An extractor is a function that extracts data from a request. The main
//! benefit of using an extractor is that it can be used directly as a parameter
//! in a route handler.
//!
//! An extractor implements either [`FromRequest`] or [`FromRequestHead`].
//! There are two variants because the request body can only be read once, so it
//! needs to be read in the [`FromRequest`] implementation. Therefore, there can
//! only be one extractor that implements [`FromRequest`] per route handler.
//!
//! # Examples
//!
//! For example, the [`Path`] extractor is used to extract path parameters:
//!
//! ```
//! use cot::html::Html;
//! use cot::request::extractors::{FromRequest, Path};
//! use cot::request::{Request, RequestExt};
//! use cot::router::{Route, Router};
//! use cot::test::TestRequestBuilder;
//!
//! async fn my_handler(Path(my_param): Path<String>) -> Html {
//!     Html::new(format!("Hello {my_param}!"))
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> cot::Result<()> {
//! let router = Router::with_urls([Route::with_handler_and_name(
//!     "/{my_param}/",
//!     my_handler,
//!     "home",
//! )]);
//! let request = TestRequestBuilder::get("/world/")
//!     .router(router.clone())
//!     .build();
//!
//! assert_eq!(
//!     router
//!         .handle(request)
//!         .await?
//!         .into_body()
//!         .into_bytes()
//!         .await?,
//!     "Hello world!"
//! );
//! # Ok(())
//! # }
//! ```

use std::future::Future;

use serde::de::DeserializeOwned;

#[cfg(feature = "json")]
use crate::json::Json;
use crate::request::{InvalidContentType, PathParams, Request, RequestHead};
use crate::{Body, Method};

pub trait FromRequest: Sized {
    /// Extracts data from the request.
    ///
    /// # Errors
    ///
    /// Throws an error if the extractor fails to extract the data from the
    /// request.
    fn from_request(
        head: &RequestHead,
        body: Body,
    ) -> impl Future<Output = crate::Result<Self>> + Send;
}

impl FromRequest for Request {
    async fn from_request(head: &RequestHead, body: Body) -> crate::Result<Self> {
        Ok(Request::from_parts(head.clone(), body))
    }
}

/// extractors.
pub trait FromRequestHead: Sized {
    /// Extracts data from the request head.
    ///
    /// # Errors
    ///
    /// Throws an error if the extractor fails to extract the data from the
    /// request head.
    fn from_request_head(head: &RequestHead) -> impl Future<Output = crate::Result<Self>> + Send;
}

/// An extractor that extracts data from the URL params.
///
/// The extractor is generic over a type that implements
/// [`DeserializeOwned`].
///
/// # Examples
///
/// ```
/// use cot::html::Html;
/// use cot::request::extractors::{FromRequest, Path};
/// use cot::request::{Request, RequestExt};
/// use cot::router::{Route, Router};
/// use cot::test::TestRequestBuilder;
///
/// async fn my_handler(Path(my_param): Path<String>) -> Html {
///     Html::new(format!("Hello {my_param}!"))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let router = Router::with_urls([Route::with_handler_and_name(
///     "/{my_param}/",
///     my_handler,
///     "home",
/// )]);
/// let request = TestRequestBuilder::get("/world/")
///     .router(router.clone())
///     .build();
///
/// assert_eq!(
///     router
///         .handle(request)
///         .await?
///         .into_body()
///         .into_bytes()
///         .await?,
///     "Hello world!"
/// );
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Path<D>(pub D);

impl<D: DeserializeOwned> FromRequestHead for Path<D> {
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        let params = head
            .extensions
            .get::<PathParams>()
            .expect("PathParams extension missing")
            .parse()?;
        Ok(Self(params))
    }
}

/// An extractor that extracts data from the URL query parameters.
///
/// The extractor is generic over a type that implements
/// [`DeserializeOwned`].
///
/// # Example
///
/// ```
/// use cot::RequestHandler;
/// use cot::html::Html;
/// use cot::request::extractors::{FromRequest, UrlQuery};
/// use cot::router::{Route, Router};
/// use cot::test::TestRequestBuilder;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct MyQuery {
///     hello: String,
/// }
///
/// async fn my_handler(UrlQuery(query): UrlQuery<MyQuery>) -> Html {
///     Html::new(format!("Hello {}!", query.hello))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let request = TestRequestBuilder::get("/?hello=world").build();
///
/// assert_eq!(
///     my_handler
///         .handle(request)
///         .await?
///         .into_body()
///         .into_bytes()
///         .await?,
///     "Hello world!"
/// );
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct UrlQuery<T>(pub T);

impl<D: DeserializeOwned> FromRequestHead for UrlQuery<D>
where
    D: DeserializeOwned,
{
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        let query = head.uri.query().unwrap_or_default();

        let deserializer =
            serde_html_form::Deserializer::new(form_urlencoded::parse(query.as_bytes()));

        let value =
            serde_path_to_error::deserialize(deserializer).map_err(QueryParametersParseError)?;

        Ok(UrlQuery(value))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("could not parse query parameters: {0}")]
struct QueryParametersParseError(serde_path_to_error::Error<serde::de::value::Error>);
impl_into_cot_error!(QueryParametersParseError, BAD_REQUEST);

/// Extractor that gets the request body as JSON and deserializes it into a type
/// `T` implementing [`DeserializeOwned`].
///
/// The content type of the request must be `application/json`.
///
/// # Errors
///
/// Throws an error if the content type is not `application/json`.
/// Throws an error if the request body could not be read.
/// Throws an error if the request body could not be deserialized - either
/// because the JSON is invalid or because the deserialization to the target
/// structure failed.
///
/// # Example
///
/// ```
/// use cot::RequestHandler;
/// use cot::json::Json;
/// use cot::test::TestRequestBuilder;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct MyData {
///     hello: String,
/// }
///
/// async fn my_handler(Json(data): Json<MyData>) -> Json<MyData> {
///     Json(data)
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let request = TestRequestBuilder::get("/")
///     .json(&MyData {
///         hello: "world".to_string(),
///     })
///     .build();
///
/// assert_eq!(
///     my_handler
///         .handle(request)
///         .await?
///         .into_body()
///         .into_bytes()
///         .await?,
///     "{\"hello\":\"world\"}"
/// );
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "json")]
impl<D: DeserializeOwned> FromRequest for Json<D> {
    async fn from_request(head: &RequestHead, body: Body) -> crate::Result<Self> {
        let content_type = head
            .headers
            .get(http::header::CONTENT_TYPE)
            .map_or("".into(), |value| String::from_utf8_lossy(value.as_bytes()));
        if content_type != crate::headers::JSON_CONTENT_TYPE {
            return Err(InvalidContentType {
                expected: crate::headers::JSON_CONTENT_TYPE,
                actual: content_type.into_owned(),
            }
            .into());
        }

        let bytes = body.into_bytes().await?;

        let deserializer = &mut serde_json::Deserializer::from_slice(&bytes);
        let result =
            serde_path_to_error::deserialize(deserializer).map_err(JsonDeserializeError)?;

        Ok(Self(result))
    }
}

#[cfg(feature = "json")]
#[derive(Debug, thiserror::Error)]
#[error("JSON deserialization error: {0}")]
struct JsonDeserializeError(serde_path_to_error::Error<serde_json::Error>);
#[cfg(feature = "json")]
impl_into_cot_error!(JsonDeserializeError, BAD_REQUEST);

// extractor impls for existing types
impl FromRequestHead for RequestHead {
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        Ok(head.clone())
    }
}

impl FromRequestHead for Method {
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        Ok(head.method.clone())
    }
}

/// A derive macro that automatically implements the [`FromRequestHead`] trait
/// for structs.
///
/// This macro generates code to extract each field of the struct from HTTP
/// request head, making it easy to create composite extractors that combine
/// multiple data sources from an incoming request.
///
/// The macro works by calling [`FromRequestHead::from_request_head`] on each
/// field's type, allowing you to compose extractors seamlessly. All fields must
/// implement the [`FromRequestHead`] trait for the derivation to work.
///
/// # Requirements
///
/// - The target struct must have all fields implement [`FromRequestHead`]
/// - Works with named fields, unnamed fields (tuple structs), and unit structs
/// - The struct must be accessible where the macro is used
///
/// # Examples
///
/// ## Named Fields
///
/// ```no_run
/// use cot::request::extractors::{Path, StaticFiles, UrlQuery};
/// use cot::router::Urls;
/// use cot_macros::FromRequestHead;
/// use serde::Deserialize;
///
/// #[derive(Debug, FromRequestHead)]
/// pub struct BaseContext {
///     urls: Urls,
///     static_files: StaticFiles,
/// }
/// ```
pub use cot_macros::FromRequestHead;

use crate::error::impl_into_cot_error;

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::request::extractors::{FromRequest, Json, Path, UrlQuery};

    #[cfg(feature = "json")]
    #[cot::test]
    async fn json() {
        let request = http::Request::builder()
            .method(http::Method::POST)
            .header(
                http::header::CONTENT_TYPE,
                crate::headers::JSON_CONTENT_TYPE,
            )
            .body(Body::fixed(r#"{"hello":"world"}"#))
            .unwrap();

        let (head, body) = request.into_parts();
        let Json(data): Json<serde_json::Value> = Json::from_request(&head, body).await.unwrap();
        assert_eq!(data, serde_json::json!({"hello": "world"}));
    }

    #[cfg(feature = "json")]
    #[cot::test]
    async fn json_empty() {
        #[derive(Debug, Deserialize, PartialEq, Eq)]
        struct TestData {}

        let request = http::Request::builder()
            .method(http::Method::POST)
            .header(
                http::header::CONTENT_TYPE,
                crate::headers::JSON_CONTENT_TYPE,
            )
            .body(Body::fixed("{}"))
            .unwrap();

        let (head, body) = request.into_parts();
        let Json(data): Json<TestData> = Json::from_request(&head, body).await.unwrap();
        assert_eq!(data, TestData {});
    }

    #[cfg(feature = "json")]
    #[cot::test]
    async fn json_struct() {
        #[derive(Debug, Deserialize, PartialEq, Eq)]
        struct TestDataInner {
            hello: String,
        }

        #[derive(Debug, Deserialize, PartialEq, Eq)]
        struct TestData {
            inner: TestDataInner,
        }

        let request = http::Request::builder()
            .method(http::Method::POST)
            .header(
                http::header::CONTENT_TYPE,
                crate::headers::JSON_CONTENT_TYPE,
            )
            .body(Body::fixed(r#"{"inner":{"hello":"world"}}"#))
            .unwrap();

        let (head, body) = request.into_parts();
        let Json(data): Json<TestData> = Json::from_request(&head, body).await.unwrap();
        assert_eq!(
            data,
            TestData {
                inner: TestDataInner {
                    hello: "world".to_string(),
                }
            }
        );
    }

    #[cot::test]
    async fn path_extraction() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct TestParams {
            id: i32,
            name: String,
        }

        let (mut head, _body) = Request::new(Body::empty()).into_parts();

        let mut params = PathParams::new();
        params.insert("id".to_string(), "42".to_string());
        params.insert("name".to_string(), "test".to_string());
        head.extensions.insert(params);

        let Path(extracted): Path<TestParams> = Path::from_request_head(&head).await.unwrap();
        let expected = TestParams {
            id: 42,
            name: "test".to_string(),
        };

        assert_eq!(extracted, expected);
    }

    #[cot::test]
    async fn url_query_extraction() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct QueryParams {
            page: i32,
            filter: String,
        }

        let (mut head, _body) = Request::new(Body::empty()).into_parts();
        head.uri = "https://example.com/?page=2&filter=active".parse().unwrap();

        let UrlQuery(query): UrlQuery<QueryParams> =
            UrlQuery::from_request_head(&head).await.unwrap();

        assert_eq!(query.page, 2);
        assert_eq!(query.filter, "active");
    }

    #[cot::test]
    async fn url_query_empty() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct EmptyParams {}

        let (mut head, _body) = Request::new(Body::empty()).into_parts();
        head.uri = "https://example.com/".parse().unwrap();

        let result: UrlQuery<EmptyParams> = UrlQuery::from_request_head(&head).await.unwrap();
        assert!(matches!(result, UrlQuery(_)));
    }

    #[cfg(feature = "json")]
    #[cot::test]
    async fn json_invalid_content_type() {
        let request = http::Request::builder()
            .method(http::Method::POST)
            .header(http::header::CONTENT_TYPE, "text/plain")
            .body(Body::fixed(r#"{"hello":"world"}"#))
            .unwrap();

        let (head, body) = request.into_parts();
        let result = Json::<serde_json::Value>::from_request(&head, body).await;
        assert!(result.is_err());
    }
}
