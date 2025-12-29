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
//! use cot::router::{Route, Router};
//! use cot::test::TestRequestBuilder;
//! use cot_core::request::extractors::{FromRequest, Path};
//! use cot_core::request::{Request, RequestExt};
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
use tower_sessions::Session;

pub use crate::request::{PathParams, Request, RequestHead};
use crate::{Body, Method};

/// Trait for extractors that consume the request body.
///
/// Extractors implementing this trait are used in route handlers that consume
/// the request body and therefore can only be used once per request.
///
/// See [`crate::request::extractors`] documentation for more information about
/// extractors.
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

/// Trait for extractors that don't consume the request body.
///
/// Extractors implementing this trait are used in route handlers that don't
/// consume the request and therefore can be used multiple times per request.
///
/// If you need to consume the body of the request, use [`FromRequest`] instead.
///
/// See [`crate::request::extractors`] documentation for more information about
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
/// `serde::de::DeserializeOwned`.
///
/// # Examples
///
/// ```
/// use cot::html::Html;
/// use cot::router::{Route, Router};
/// use cot::test::TestRequestBuilder;
/// use cot_core::request::extractors::{FromRequest, Path};
/// use cot_core::request::{Request, RequestExt};
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
/// `serde::de::DeserializeOwned`.
///
/// # Example
///
/// ```
/// use cot::RequestHandler;
/// use cot::html::Html;
/// use cot::router::{Route, Router};
/// use cot::test::TestRequestBuilder;
/// use cot_core::request::extractors::{FromRequest, UrlQuery};
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

impl FromRequestHead for Session {
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        Ok(Session::from_extensions(&head.extensions).clone())
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
/// use cot::router::Urls;
/// use cot_core::request::extractors::{Path, StaticFiles, UrlQuery};
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


use crate::impl_into_cot_error;

#[cfg(test)]
mod tests {
    use cot::html::Html;
    use cot::router::{Route, Router, Urls};
    use cot::test::TestRequestBuilder;
    use serde::Deserialize;

    use super::*;
    use crate::request::extractors::{FromRequest, Path, UrlQuery};

    #[cot_macros::test]
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

    #[cot_macros::test]
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

    #[cot_macros::test]
    async fn url_query_empty() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct EmptyParams {}

        let (mut head, _body) = Request::new(Body::empty()).into_parts();
        head.uri = "https://example.com/".parse().unwrap();

        let result: UrlQuery<EmptyParams> = UrlQuery::from_request_head(&head).await.unwrap();
        assert!(matches!(result, UrlQuery(_)));
    }

    #[cot_macros::test]
    async fn request_form() {
        #[derive(Debug, PartialEq, Eq, Form)]
        struct MyForm {
            hello: String,
            foo: String,
        }

        let request = TestRequestBuilder::post("/")
            .form_data(&[("hello", "world"), ("foo", "bar")])
            .build();

        let (head, body) = request.into_parts();
        let RequestForm(form_result): RequestForm<MyForm> =
            RequestForm::from_request(&head, body).await.unwrap();

        assert_eq!(
            form_result.unwrap(),
            MyForm {
                hello: "world".to_string(),
                foo: "bar".to_string(),
            }
        );
    }

    #[cot_macros::test]
    async fn urls_extraction() {
        async fn handler() -> Html {
            Html::new("")
        }

        let router = Router::with_urls([Route::with_handler_and_name(
            "/test/",
            handler,
            "test_route",
        )]);

        let mut request = TestRequestBuilder::get("/test/").router(router).build();

        let urls: Urls = request.extract_from_head().await.unwrap();

        assert!(reverse!(urls, "test_route").is_ok());
    }

    #[cot_macros::test]
    async fn method_extraction() {
        let mut request = TestRequestBuilder::get("/test/").build();

        let method: Method = request.extract_from_head().await.unwrap();

        assert_eq!(method, Method::GET);
    }
}
