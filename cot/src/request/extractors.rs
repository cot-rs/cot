//! Extractors for request data.
//!
//! An extractor is a function that extracts data from a request. The main
//! benefit of using an extractor is that it can be used directly as a parameter
//! in a route handler.
//!
//! An extractor implements either [`FromRequest`] or [`FromRequestParts`].
//! There are two variants because the request body can only be read once, so it
//! needs to be read in the [`FromRequest`] implementation. Therefore, there can
//! only be one extractor that implements [`FromRequest`] per route handler.
//!
//! # Examples
//!
//! For example, the [`Path`] extractor is used to extract path parameters:
//!
//! ```
//! use cot::request::extractors::{FromRequest, Path};
//! use cot::request::{Request, RequestExt};
//! use cot::response::{Response, ResponseExt};
//! use cot::router::{Route, Router};
//! use cot::test::TestRequestBuilder;
//! use cot::{Body, RequestHandler};
//!
//! async fn my_handler(Path(my_param): Path<String>) -> cot::Result<Response> {
//!     Ok(Response::new_html(
//!         cot::StatusCode::OK,
//!         Body::fixed(format!("Hello {my_param}!")),
//!     ))
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

use cot::Error;
use cot::error::ErrorRepr;
use cot::request::{PathParams, Request};
use http::request::Parts;
use serde::de::DeserializeOwned;

use crate::Method;
use crate::auth::Auth;
use crate::form::{Form, FormResult};
use crate::request::RequestExt;
use crate::router::Urls;
use crate::session::Session;

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
    fn from_request(request: Request) -> impl Future<Output = cot::Result<Self>> + Send;
}

impl FromRequest for Request {
    async fn from_request(request: Request) -> cot::Result<Self> {
        Ok(request)
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
pub trait FromRequestParts: Sized {
    /// Extracts data from the request parts.
    ///
    /// # Errors
    ///
    /// Throws an error if the extractor fails to extract the data from the
    /// request parts.
    fn from_request_parts(parts: &mut Parts) -> impl Future<Output = cot::Result<Self>> + Send;
}

impl FromRequestParts for Urls {
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        Ok(Self::from_parts(parts))
    }
}

/// An extractor that extract data from the URL params.
///
/// The extractor is generic over a type that implements
/// `serde::de::DeserializeOwned`.
///
/// # Examples
///
/// ```
/// use cot::request::extractors::{FromRequest, Path};
/// use cot::request::{Request, RequestExt};
/// use cot::response::{Response, ResponseExt};
/// use cot::router::{Route, Router};
/// use cot::test::TestRequestBuilder;
/// use cot::{Body, RequestHandler};
///
/// async fn my_handler(Path(my_param): Path<String>) -> cot::Result<Response> {
///     Ok(Response::new_html(
///         cot::StatusCode::OK,
///         Body::fixed(format!("Hello {my_param}!")),
///     ))
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

impl<D: DeserializeOwned> FromRequestParts for Path<D> {
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        let params = parts
            .extensions
            .get::<PathParams>()
            .expect("PathParams extension missing")
            .parse()
            .map_err(|error| Error::new(ErrorRepr::PathParametersParse(error)))?;
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
/// use cot::request::extractors::{FromRequest, UrlQuery};
/// use cot::request::{Request, RequestExt};
/// use cot::response::{Response, ResponseExt};
/// use cot::router::{Route, Router};
/// use cot::test::TestRequestBuilder;
/// use cot::{Body, RequestHandler};
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct MyQuery {
///     hello: String,
/// }
///
/// async fn my_handler(UrlQuery(query): UrlQuery<MyQuery>) -> cot::Result<Response> {
///     Ok(Response::new_html(
///         cot::StatusCode::OK,
///         Body::fixed(format!("Hello {}!", query.hello)),
///     ))
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

impl<D: DeserializeOwned> FromRequestParts for UrlQuery<D>
where
    D: DeserializeOwned,
{
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        let query = parts.uri.query().unwrap_or_default();

        let deserializer =
            serde_html_form::Deserializer::new(form_urlencoded::parse(query.as_bytes()));

        let value = serde_path_to_error::deserialize(deserializer)
            .map_err(|error| Error::new(ErrorRepr::QueryParametersParse(error)))?;

        Ok(UrlQuery(value))
    }
}

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
/// use cot::request::extractors::Json;
/// use cot::request::{Request, RequestExt};
/// use cot::response::{Response, ResponseExt};
/// use cot::test::TestRequestBuilder;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct MyData {
///     hello: String,
/// }
///
/// async fn my_handler(Json(data): Json<MyData>) -> cot::Result<Response> {
///     Ok(Response::new_json(cot::StatusCode::OK, &data)?)
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
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Json<D>(pub D);

#[cfg(feature = "json")]
impl<D: DeserializeOwned> FromRequest for Json<D> {
    async fn from_request(mut request: Request) -> cot::Result<Self> {
        request.expect_content_type(cot::headers::JSON_CONTENT_TYPE)?;

        let body = std::mem::take(request.body_mut());
        let bytes = body.into_bytes().await?;

        let deserializer = &mut serde_json::Deserializer::from_slice(&bytes);
        let result = serde_path_to_error::deserialize(deserializer)
            .map_err(|error| Error::new(ErrorRepr::Json(error)))?;

        Ok(Self(result))
    }
}

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
/// use cot::request::extractors::RequestForm;
/// use cot::request::{Request, RequestExt};
/// use cot::response::{Response, ResponseExt};
/// use cot::test::TestRequestBuilder;
/// use cot::{Body, RequestHandler};
/// use serde::Deserialize;
///
/// #[derive(Form)]
/// struct MyForm {
///     hello: String,
/// }
///
/// async fn my_handler(RequestForm(form): RequestForm<MyForm>) -> cot::Result<Response> {
///     let form = match form {
///         FormResult::Ok(form) => form,
///         FormResult::ValidationError(error) => {
///             panic!("Form validation error!")
///         }
///     };
///
///     Ok(Response::new_html(
///         cot::StatusCode::OK,
///         Body::fixed(format!("Hello {}!", form.hello)),
///     ))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// # let request = TestRequestBuilder::post("/").form_data(&[("hello", "world")]).build();
/// # my_handler.handle(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct RequestForm<F: Form>(pub FormResult<F>);

impl<F: Form> FromRequest for RequestForm<F> {
    async fn from_request(mut request: Request) -> cot::Result<Self> {
        Ok(Self(F::from_request(&mut request).await?))
    }
}

/// An extractor that gets the database from the request extensions.
///
/// # Example
///
/// ```
/// use cot::request::extractors::RequestDb;
/// use cot::request::{Request, RequestExt};
/// use cot::response::{Response, ResponseExt};
/// use cot::test::{TestDatabase, TestRequestBuilder};
/// use cot::{Body, RequestHandler};
///
/// async fn my_handler(RequestDb(db): RequestDb) -> cot::Result<Response> {
///     // ... do something with the database
///     # db.close().await?;
///     # Ok(Response::new_html(cot::StatusCode::OK, Body::empty()))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// # let request = TestRequestBuilder::get("/")
/// #     .database(TestDatabase::new_sqlite().await?.database())
/// #     .build();
/// # my_handler.handle(request).await?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "db")]
#[derive(Debug)]
pub struct RequestDb(pub std::sync::Arc<crate::db::Database>);

#[cfg(feature = "db")]
impl FromRequestParts for RequestDb {
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        Ok(Self(parts.db().clone()))
    }
}

// extractor impls for existing types
impl FromRequestParts for Method {
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        Ok(parts.method.clone())
    }
}

impl FromRequestParts for Session {
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        Ok(Session::from_extensions(&parts.extensions).clone())
    }
}

impl FromRequestParts for Auth {
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        let auth = parts
            .extensions
            .get::<Auth>()
            .expect("Auth extension missing. Did you forget to add the AuthMiddleware?")
            .clone();

        Ok(auth)
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::request::extractors::{FromRequest, Json, Path, UrlQuery};
    use crate::response::{Response, ResponseExt};
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

        let Json(data): Json<serde_json::Value> = Json::from_request(request).await.unwrap();
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

        let Json(data): Json<TestData> = Json::from_request(request).await.unwrap();
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

        let Json(data): Json<TestData> = Json::from_request(request).await.unwrap();
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

        let (mut parts, _body) = Request::new(Body::empty()).into_parts();

        let mut params = PathParams::new();
        params.insert("id".to_string(), "42".to_string());
        params.insert("name".to_string(), "test".to_string());
        parts.extensions.insert(params);

        let Path(extracted): Path<TestParams> = Path::from_request_parts(&mut parts).await.unwrap();
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

        let (mut parts, _body) = Request::new(Body::empty()).into_parts();
        parts.uri = "https://example.com/?page=2&filter=active".parse().unwrap();

        let UrlQuery(query): UrlQuery<QueryParams> =
            UrlQuery::from_request_parts(&mut parts).await.unwrap();

        assert_eq!(query.page, 2);
        assert_eq!(query.filter, "active");
    }

    #[cot::test]
    async fn url_query_empty() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct EmptyParams {}

        let (mut parts, _body) = Request::new(Body::empty()).into_parts();
        parts.uri = "https://example.com/".parse().unwrap();

        let result: UrlQuery<EmptyParams> = UrlQuery::from_request_parts(&mut parts).await.unwrap();
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

        let result = Json::<serde_json::Value>::from_request(request).await;
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

        let RequestForm(form_result): RequestForm<MyForm> =
            RequestForm::from_request(request).await.unwrap();

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
        async fn handler() -> cot::Result<Response> {
            Ok(Response::new_html(cot::StatusCode::OK, Body::empty()))
        }

        let router = Router::with_urls([Route::with_handler_and_name(
            "/test/",
            handler,
            "test_route",
        )]);

        let mut request = TestRequestBuilder::get("/test/").router(router).build();

        let urls: Urls = request.extract_parts().await.unwrap();

        assert!(reverse!(urls, "test_route").is_ok());
    }

    #[cot::test]
    async fn method_extraction() {
        let mut request = TestRequestBuilder::get("/test/").build();

        let method: Method = request.extract_parts().await.unwrap();

        assert_eq!(method, Method::GET);
    }

    #[cfg(feature = "db")]
    #[cot::test]
    // unsupported operation: can't call foreign function `sqlite3_open_v2` on OS `linux`
    #[cfg_attr(miri, ignore)]
    async fn request_db() {
        let db = crate::test::TestDatabase::new_sqlite().await.unwrap();
        let mut test_request = TestRequestBuilder::get("/").database(db.database()).build();

        let RequestDb(extracted_db) = test_request.extract_parts().await.unwrap();

        // check that we have a connection to the database
        extracted_db.close().await.unwrap();
    }
}
