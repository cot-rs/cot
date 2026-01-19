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
use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::auth::Auth;
use crate::form::{Form, FormResult};
#[cfg(feature = "json")]
use crate::json::Json;
use crate::request::{InvalidContentType, PathParams, Request, RequestExt, RequestHead};
use crate::router::Urls;
use crate::session::Session;
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
    ) -> impl Future<Output = cot::Result<Self>> + Send;
}

impl FromRequest for Request {
    async fn from_request(head: &RequestHead, body: Body) -> cot::Result<Self> {
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
    fn from_request_head(head: &RequestHead) -> impl Future<Output = cot::Result<Self>> + Send;
}

impl FromRequestHead for Urls {
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(Self::from_parts(head))
    }
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
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
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
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
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
/// `T` implementing `serde::de::DeserializeOwned`.
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
    async fn from_request(head: &RequestHead, body: Body) -> cot::Result<Self> {
        let content_type = head
            .headers
            .get(http::header::CONTENT_TYPE)
            .map_or("".into(), |value| String::from_utf8_lossy(value.as_bytes()));
        if content_type != cot::headers::JSON_CONTENT_TYPE {
            return Err(InvalidContentType {
                expected: cot::headers::JSON_CONTENT_TYPE,
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

/// An extractor that gets the request body as form data and deserializes it
/// into a type `F` implementing `cot::form::Form`.
///
/// The content type of the request must be `application/x-www-form-urlencoded`.
///
/// # Errors
///
/// Throws an error if the content type is not
/// `application/x-www-form-urlencoded`. Throws an error if the request body
/// could not be read. Throws an error if the request body could not be
/// deserialized - either because the form data is invalid or because the
/// deserialization to the target structure failed.
///
/// # Example
///
/// ```
/// use cot::form::{Form, FormResult};
/// use cot::html::Html;
/// use cot::request::extractors::RequestForm;
/// use cot::test::TestRequestBuilder;
///
/// #[derive(Form)]
/// struct MyForm {
///     hello: String,
/// }
///
/// async fn my_handler(RequestForm(form): RequestForm<MyForm>) -> Html {
///     let form = match form {
///         FormResult::Ok(form) => form,
///         FormResult::ValidationError(error) => {
///             panic!("Form validation error!")
///         }
///     };
///
///     Html::new(format!("Hello {}!", form.hello))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// # use cot::RequestHandler;
/// # let request = TestRequestBuilder::post("/").form_data(&[("hello", "world")]).build();
/// # my_handler.handle(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct RequestForm<F: Form>(pub FormResult<F>);

impl<F: Form> FromRequest for RequestForm<F> {
    async fn from_request(head: &RequestHead, body: Body) -> cot::Result<Self> {
        let mut request = Request::from_parts(head.clone(), body);
        Ok(Self(F::from_request(&mut request).await?))
    }
}

#[cfg(feature = "db")]
impl FromRequestHead for crate::db::Database {
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(head.context().database().clone())
    }
}

#[cfg(feature = "cache")]
impl FromRequestHead for crate::cache::Cache {
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(head.context().cache().clone())
    }
}

#[cfg(feature = "email")]
impl FromRequestHead for crate::email::Email {
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(head.context().email().clone())
    }
}

/// An extractor that allows you to access static files metadata (e.g., their
/// URLs).
///
/// # Examples
///
/// ```
/// use cot::html::Html;
/// use cot::request::Request;
/// use cot::request::extractors::StaticFiles;
/// use cot::test::TestRequestBuilder;
///
/// async fn my_handler(static_files: StaticFiles) -> cot::Result<Html> {
///     let url = static_files.url_for("css/main.css")?;
///
///     Ok(Html::new(format!(
///         "<html><head><link rel=\"stylesheet\" href=\"{url}\"></head></html>"
///     )))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// # use cot::RequestHandler;
/// # let request = TestRequestBuilder::get("/")
/// #     .static_file("css/main.css", "body { color: red; }")
/// #     .build();
/// # my_handler.handle(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticFiles {
    inner: Arc<crate::static_files::StaticFiles>,
}

impl StaticFiles {
    /// Gets the URL for a static file.
    ///
    /// This method returns the URL that can be used to access the static file.
    /// The URL is constructed based on the static files configuration, which
    /// may include a URL prefix or be suffixed by a content hash.
    ///
    /// # Errors
    ///
    /// Returns a [`StaticFilesGetError::NotFound`] error if the file doesn't
    /// exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::Html;
    /// use cot::request::extractors::StaticFiles;
    /// use cot::test::TestRequestBuilder;
    ///
    /// async fn my_handler(static_files: StaticFiles) -> cot::Result<Html> {
    ///     let url = static_files.url_for("css/main.css")?;
    ///
    ///     Ok(Html::new(format!(
    ///         "<html><head><link rel=\"stylesheet\" href=\"{url}\"></head></html>"
    ///     )))
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// # use cot::RequestHandler;
    /// # let request = TestRequestBuilder::get("/")
    /// #     .static_file("css/main.css", "body { color: red; }")
    /// #     .build();
    /// # my_handler.handle(request).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn url_for(&self, path: &str) -> Result<&str, StaticFilesGetError> {
        self.inner
            .path_for(path)
            .ok_or_else(|| StaticFilesGetError::NotFound {
                path: path.to_owned(),
            })
    }
}

const ERROR_PREFIX: &str = "could not get URL for a static file:";
/// Errors that can occur when trying to get a static file.
///
/// This enum represents errors that can occur when attempting to
/// access a static file through the [`StaticFiles`] extractor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum StaticFilesGetError {
    /// The requested static file was not found.
    #[error("{ERROR_PREFIX} static file `{path}` not found")]
    #[non_exhaustive]
    NotFound {
        /// The path of the static file that was not found.
        path: String,
    },
}
impl_into_cot_error!(StaticFilesGetError);

impl FromRequestHead for StaticFiles {
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(StaticFiles {
            inner: head
                .extensions
                .get::<Arc<crate::static_files::StaticFiles>>()
                .cloned()
                .expect("StaticFilesMiddleware not enabled for the route/project"),
        })
    }
}

// extractor impls for existing types
impl FromRequestHead for RequestHead {
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(head.clone())
    }
}

impl FromRequestHead for Method {
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(head.method.clone())
    }
}

impl FromRequestHead for Session {
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(Session::from_extensions(&head.extensions).clone())
    }
}

impl FromRequestHead for Auth {
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        let auth = head
            .extensions
            .get::<Auth>()
            .expect("AuthMiddleware not enabled for the route/project")
            .clone();

        Ok(auth)
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

use crate::error::error_impl::impl_into_cot_error;

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::html::Html;
    use crate::request::extractors::{FromRequest, Json, Path, UrlQuery};
    use crate::router::{Route, Router, Urls};
    use crate::test::TestRequestBuilder;
    use crate::{Body, reverse};

    #[cfg(feature = "json")]
    #[cot::test]
    async fn json() {
        let request = http::Request::builder()
            .method(http::Method::POST)
            .header(http::header::CONTENT_TYPE, cot::headers::JSON_CONTENT_TYPE)
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
            .header(http::header::CONTENT_TYPE, cot::headers::JSON_CONTENT_TYPE)
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
            .header(http::header::CONTENT_TYPE, cot::headers::JSON_CONTENT_TYPE)
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

    #[cot::test]
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

    #[cot::test]
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

    #[cot::test]
    async fn method_extraction() {
        let mut request = TestRequestBuilder::get("/test/").build();

        let method: Method = request.extract_from_head().await.unwrap();

        assert_eq!(method, Method::GET);
    }

    #[cfg(feature = "db")]
    #[cot::test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2` on OS `linux`"
    )]
    async fn request_db() {
        let db = crate::test::TestDatabase::new_sqlite().await.unwrap();
        let mut test_request = TestRequestBuilder::get("/").database(db.database()).build();

        let extracted_db: crate::db::Database = test_request.extract_from_head().await.unwrap();

        // check that we have a connection to the database
        extracted_db.close().await.unwrap();
    }

    #[cfg(feature = "cache")]
    #[cot::test]
    async fn request_cache() {
        let mut request_builder = TestRequestBuilder::get("/");
        let mut request = request_builder.build();

        let extracted_cache = request.extract_from_head::<crate::cache::Cache>().await;
        assert!(extracted_cache.is_ok());
    }

    #[cfg(feature = "email")]
    #[cot::test]
    async fn request_email() {
        let mut request_builder = TestRequestBuilder::get("/");
        let mut request = request_builder.build();

        let email_service = request.extract_from_head::<crate::email::Email>().await;
        assert!(email_service.is_ok());
    }
}
