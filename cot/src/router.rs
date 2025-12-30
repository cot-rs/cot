use std::sync::Arc;

use cot_core::request::{Request, RequestHead};
#[doc(inline)]
pub use cot_core::router::{Route, Router, method::MethodRouter};

use crate::request::RequestExt;

/// A helper structure to allow reversing URLs from a request handler.
///
/// This is mainly useful as an extractor to allow reversing URLs without
/// access to a full [`Request`] object.
///
/// # Examples
///
/// ```
/// use cot::html::Html;
/// use cot::test::TestRequestBuilder;
/// use cot::{RequestHandler, reverse};
/// use cot_core::router::{Route, Router, Urls};
///
/// async fn my_handler(urls: Urls) -> cot::Result<Html> {
///     let url = reverse!(urls, "home")?;
///     Ok(Html::new(format!("{url}")))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let router = Router::with_urls([Route::with_handler_and_name("/", my_handler, "home")]);
/// let request = TestRequestBuilder::get("/").router(router).build();
///
/// assert_eq!(
///     my_handler
///         .handle(request)
///         .await?
///         .into_body()
///         .into_bytes()
///         .await?,
///     "/"
/// );
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Urls {
    app_name: Option<String>,
    router: Arc<Router>,
}

impl Urls {
    /// Create a new `Urls` object from a [`Request`] object.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::Html;
    /// use cot::{Body, StatusCode, reverse};
    /// use cot_core::request::Request;
    /// use cot_core::response::{Response, ResponseExt};
    /// use cot_core::router::Urls;
    ///
    /// async fn my_handler(request: Request) -> cot::Result<Html> {
    ///     let urls = Urls::from_request(&request);
    ///     let url = reverse!(urls, "home")?;
    ///     Ok(Html::new(format!(
    ///         "Hello! The URL for this view is: {}",
    ///         url
    ///     )))
    /// }
    /// ```
    pub fn from_request(request: &Request) -> Self {
        Self {
            app_name: request.app_name().map(ToOwned::to_owned),
            router: Arc::clone(request.router()),
        }
    }

    pub fn from_parts(request_head: &RequestHead) -> Self {
        Self {
            app_name: request_head.app_name().map(ToOwned::to_owned),
            router: Arc::clone(request_head.router()),
        }
    }

    /// Get the app name the current route belongs to, or [`None`] if the
    /// request is not routed.
    ///
    /// This is mainly useful for providing context to reverse redirects, where
    /// you want to redirect to a route in the same app.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot_core::request::{Request, RequestExt};
    /// use cot_core::response::Response;
    /// use cot_core::router::Urls;
    ///
    /// async fn my_handler(urls: Urls) -> cot::Result<Response> {
    ///     let app_name = urls.app_name();
    ///     // ... do something with the app name
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    pub fn app_name(&self) -> Option<&str> {
        self.app_name.as_deref()
    }

    /// Get the router.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot_core::request::{Request, RequestExt};
    /// use cot_core::response::Response;
    /// use cot_core::router::Urls;
    ///
    /// async fn my_handler(urls: Urls) -> cot::Result<Response> {
    ///     let router = urls.router();
    ///     // ... do something with the router
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    pub fn router(&self) -> &Router {
        &self.router
    }
}
