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

use std::sync::Arc;

use cot_core::error::impl_into_cot_error;
/// Trait for extractors that consume the request body.
///
/// Extractors implementing this trait are used in route handlers that consume
/// the request body and therefore can only be used once per request.
///
/// See [`crate::request::extractors`] documentation for more information about
/// extractors.
pub use cot_core::request::extractors::FromRequest;
/// Trait for extractors that don't consume the request body.
///
/// Extractors implementing this trait are used in route handlers that don't
/// consume the request and therefore can be used multiple times per request.
///
/// If you need to consume the body of the request, use [`FromRequest`] instead.
///
/// See [`crate::request::extractors`] documentation for more information about
pub use cot_core::request::extractors::FromRequestHead;
#[doc(inline)]
pub use cot_core::request::extractors::{Path, UrlQuery};

use crate::Body;
use crate::auth::Auth;
use crate::form::{Form, FormResult};
use crate::request::{Request, RequestExt, RequestHead};
use crate::router::Urls;
use crate::session::Session;

impl FromRequestHead for Urls {
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(Self::from_parts(head))
    }
}

/// An extractor that gets the request body as form data and deserializes it
/// into a type `F` implementing [`Form`].
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

#[cfg(test)]
mod tests {
    use cot_core::Method;
    use cot_core::html::Html;

    use super::*;
    use crate::request::extractors::FromRequest;
    use crate::reverse;
    use crate::router::{Route, Router};
    use crate::test::TestRequestBuilder;

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
