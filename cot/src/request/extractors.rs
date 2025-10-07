use std::sync::Arc;

#[cfg(feature = "json")]
use cot::json::Json;
use cot_core::request::RequestExt;
use cot_core::request::extractors::{FromRequest, FromRequestHead, RequestHead};
use cot_core::{Body, impl_into_cot_error};
use serde::de::DeserializeOwned;

#[derive(Debug, thiserror::Error)]
#[error("invalid content type; expected `{expected}`, found `{actual}`")]
pub struct InvalidContentType {
    expected: &'static str,
    actual: String,
}
impl_into_cot_error!(InvalidContentType, BAD_REQUEST);

#[cfg(feature = "json")]
impl<D: DeserializeOwned> FromRequest for Json<D> {
    async fn from_request(head: &RequestHead, body: Body) -> crate::Result<Self> {
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

/// An extractor that gets the database from the request extensions.
///
/// # Example
///
/// ```
/// use cot::html::Html;
/// use cot::request::extractors::RequestDb;
///
/// async fn my_handler(RequestDb(db): RequestDb) -> Html {
///     // ... do something with the database
///     # db.close().await.unwrap();
///     # Html::new("")
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// # use cot::RequestHandler;
/// # let request = cot::test::TestRequestBuilder::get("/")
/// #     .database(cot::test::TestDatabase::new_sqlite().await?.database())
/// #     .build();
/// # my_handler.handle(request).await?;
/// # Ok(())
/// # }
/// ```

#[cfg(feature = "db")]
#[derive(Debug)]
pub struct RequestDb(pub Arc<cot::db::Database>);

#[cfg(feature = "db")]
impl FromRequestHead for RequestDb {
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        Ok(Self(head.db().clone()))
    }
}

/// An extractor that allows you to access static files metadata (e.g., their
/// URLs).
///
/// # Examples
///
/// ```
/// use cot::html::Html;
/// use cot::request::extractors::StaticFiles;
/// use cot::test::TestRequestBuilder;
/// use cot_core::request::Request;
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
    inner: Arc<cot::static_files::StaticFiles>,
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
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        Ok(StaticFiles {
            inner: head
                .extensions
                .get::<Arc<cot::static_files::StaticFiles>>()
                .cloned()
                .expect("StaticFilesMiddleware not enabled for the route/project"),
        })
    }
}

#[cfg(test)]
mod tests {
    use cot::request::extractors::Json;
    use cot::test::TestRequestBuilder;
    use cot_core::request::extractors::{FromRequest, Path, UrlQuery};
    use serde::Deserialize;

    use super::*;

    #[cfg(feature = "json")]
    #[cot_macros::test]
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
    #[cot_macros::test]
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
    #[cot_macros::test]
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

    #[cfg(feature = "json")]
    #[cot_macros::test]
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

    #[cfg(feature = "db")]
    #[cot_macros::test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2` on OS `linux`"
    )]
    async fn request_db() {
        let db = cot::test::TestDatabase::new_sqlite().await.unwrap();
        let mut test_request = TestRequestBuilder::get("/").database(db.database()).build();

        let RequestDb(extracted_db) = test_request.extract_from_head().await.unwrap();

        // check that we have a connection to the database
        extracted_db.close().await.unwrap();
    }
}
