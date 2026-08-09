use crate::{Body, StatusCode};
mod into_response;

/// Derive macro for the [`IntoResponse`] trait.
///
/// This macro can be applied to enums to automatically implement the
/// [`IntoResponse`] trait. The enum must consist of tuple variants with
/// exactly one field each, with each variant containing a single field that
/// implements [`IntoResponse`].
///
/// # Requirements
///
/// - **Only enums are supported**: This macro will produce a compile error if
///   applied to structs or unions.
/// - **Tuple variants with one field**: Each enum variant must be a tuple
///   variant with exactly one field (e.g., `Variant(Type)`).
/// - **Field types must implement `IntoResponse`**: Each field type must
///   implement the [`IntoResponse`] trait.
///
/// # Generated Implementation
///
/// The macro generates an implementation that matches on the enum variants and
/// calls `into_response()` on the inner value:
///
/// ```compile_fail
/// impl IntoResponse for MyEnum {
///     fn into_response(self) -> cot::Result<cot::response::Response> {
///         use cot::response::IntoResponse;
///         match self {
///             Self::Variant1(inner) => inner.into_response(),
///             Self::Variant2(inner) => inner.into_response(),
///             // ... for each variant
///         }
///     }
/// }
/// ```
///
/// # Examples
///
/// ```
/// use cot::html::Html;
/// use cot::json::Json;
/// use cot::response::IntoResponse;
///
/// #[derive(IntoResponse)]
/// enum MyResponse {
///     Json(Json<String>),
///     Html(Html),
/// }
/// ```
///
/// [`IntoResponse`]: crate::response::IntoResponse
pub use cot_macros::IntoResponse;
pub use into_response::{
    IntoResponse, WithBody, WithContentType, WithExtension, WithHeader, WithStatus,
};

const RESPONSE_BUILD_FAILURE: &str = "Failed to build response";

/// HTTP response type.
pub type Response = http::Response<Body>;

/// HTTP response head type.
pub type ResponseHead = http::response::Parts;

mod private {
    pub trait Sealed {}
}

/// Extension trait for [`http::Response`] that provides helper methods for
/// working with HTTP responses.
///
/// # Sealed
///
/// This trait is sealed since it doesn't make sense to be implemented for types
/// outside the context of Cot.
pub trait ResponseExt: Sized + private::Sealed {
    /// Create a new response builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::StatusCode;
    /// use cot::response::{Response, ResponseExt};
    ///
    /// let response = Response::builder()
    ///     .status(StatusCode::OK)
    ///     .body(cot::Body::empty())
    ///     .expect("Failed to build response");
    /// ```
    #[must_use]
    fn builder() -> http::response::Builder;
}

impl private::Sealed for Response {}

impl ResponseExt for Response {
    fn builder() -> http::response::Builder {
        http::Response::builder()
    }
}

/// A redirect response.
///
/// This type creates an HTTP redirect response with a `Location` header set to
/// the specified URL. The status code depends on the constructor used;
/// [`Redirect::new`] defaults to
/// [`StatusCode::SEE_OTHER`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status/303)
/// (303), which is what you typically want after a successful form submission.
///
/// # Examples
///
/// ```
/// use cot::response::{IntoResponse, Redirect};
///
/// let redirect = Redirect::new("https://example.com");
/// let response = redirect.into_response().unwrap();
///
/// assert_eq!(response.status(), cot::StatusCode::SEE_OTHER);
/// ```
///
/// # See also
///
/// * [`crate::reverse_redirect!`] – a more ergonomic way to create redirects to
///   internal views
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    location: String,
    status_code: StatusCode,
}

impl Redirect {
    /// Creates a new redirect response to the specified location.
    ///
    /// Creates an HTTP redirect response with a status code of
    /// [`StatusCode::SEE_OTHER`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status/303)
    /// (303) and a `Location` header set to the specified URL. This is an alias
    /// for [`Redirect::see_other`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::{IntoResponse, Redirect};
    ///
    /// let redirect = Redirect::new("https://example.com");
    /// let response = redirect.into_response().unwrap();
    ///
    /// assert_eq!(response.status(), cot::StatusCode::SEE_OTHER);
    /// ```
    #[must_use]
    pub fn new<T: Into<String>>(location: T) -> Self {
        Self::see_other(location)
    }

    /// Creates a redirect that instructs the client to resubmit the request to
    /// the specified location using the `GET` method.
    ///
    /// This uses the
    /// [`StatusCode::SEE_OTHER`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status/303)
    /// (303) status code. It's the usual choice for redirecting after a
    /// successful `POST`, as it prevents the form from being resubmitted when
    /// the user refreshes the page.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::{IntoResponse, Redirect};
    ///
    /// let redirect = Redirect::see_other("/thank-you/");
    /// let response = redirect.into_response().unwrap();
    ///
    /// assert_eq!(response.status(), cot::StatusCode::SEE_OTHER);
    /// ```
    #[must_use]
    pub fn see_other<T: Into<String>>(location: T) -> Self {
        Self::with_status_code(location, StatusCode::SEE_OTHER)
    }

    /// Creates a redirect that tells the client the resource has moved
    /// permanently.
    ///
    /// This uses the
    /// [`StatusCode::PERMANENT_REDIRECT`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status/308)
    /// (308) status code, which — unlike the legacy `301 Moved Permanently` —
    /// guarantees that the request method and body are not changed by the
    /// client. Use this when a URL has changed for good, so that clients and
    /// search engines update their references.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::{IntoResponse, Redirect};
    ///
    /// let redirect = Redirect::permanent("https://example.com/new-url/");
    /// let response = redirect.into_response().unwrap();
    ///
    /// assert_eq!(response.status(), cot::StatusCode::PERMANENT_REDIRECT);
    /// ```
    #[must_use]
    pub fn permanent<T: Into<String>>(location: T) -> Self {
        Self::with_status_code(location, StatusCode::PERMANENT_REDIRECT)
    }

    /// Creates a redirect that tells the client the resource resides
    /// temporarily under a different location.
    ///
    /// This uses the
    /// [`StatusCode::TEMPORARY_REDIRECT`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status/307)
    /// (307) status code, which — unlike the legacy `302 Found` — guarantees
    /// that the request method and body are not changed by the client.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::{IntoResponse, Redirect};
    ///
    /// let redirect = Redirect::temporary("/maintenance/");
    /// let response = redirect.into_response().unwrap();
    ///
    /// assert_eq!(response.status(), cot::StatusCode::TEMPORARY_REDIRECT);
    /// ```
    #[must_use]
    pub fn temporary<T: Into<String>>(location: T) -> Self {
        Self::with_status_code(location, StatusCode::TEMPORARY_REDIRECT)
    }

    /// Creates a redirect with an explicitly specified status code.
    ///
    /// Prefer [`Redirect::see_other`], [`Redirect::permanent`], or
    /// [`Redirect::temporary`] unless you specifically need one of the legacy
    /// redirect status codes, such as `301 Moved Permanently` or `302 Found`.
    ///
    /// # Panics
    ///
    /// Panics if `status_code` is not a redirection (3xx) status code.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::StatusCode;
    /// use cot::response::{IntoResponse, Redirect};
    ///
    /// let redirect = Redirect::with_status_code(
    ///     "https://example.com/new-url/",
    ///     StatusCode::MOVED_PERMANENTLY,
    /// );
    /// let response = redirect.into_response().unwrap();
    ///
    /// assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    /// ```
    #[must_use]
    pub fn with_status_code<T: Into<String>>(location: T, status_code: StatusCode) -> Self {
        assert!(
            status_code.is_redirection(),
            "`{status_code}` is not a redirection status code"
        );

        Self {
            location: location.into(),
            status_code,
        }
    }

    /// Returns the URL this redirect points to.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Redirect;
    ///
    /// let redirect = Redirect::permanent("https://example.com/new-url/");
    ///
    /// assert_eq!(redirect.location(), "https://example.com/new-url/");
    /// ```
    #[must_use]
    pub fn location(&self) -> &str {
        &self.location
    }

    /// Returns the status code this redirect will be sent with.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Redirect;
    ///
    /// let redirect = Redirect::permanent("https://example.com/new-url/");
    ///
    /// assert_eq!(redirect.status_code(), cot::StatusCode::PERMANENT_REDIRECT);
    /// ```
    #[must_use]
    pub fn status_code(&self) -> StatusCode {
        self.status_code
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StatusCode;
    use crate::body::BodyInner;
    use crate::headers::JSON_CONTENT_TYPE;

    #[test]
    #[cfg(feature = "json")]
    fn response_new_json() {
        #[derive(serde::Serialize)]
        struct MyData {
            hello: String,
        }

        let data = MyData {
            hello: String::from("world"),
        };
        let response = crate::json::Json(data).into_response().unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            JSON_CONTENT_TYPE
        );
        match &response.body().inner {
            BodyInner::Fixed(fixed) => {
                assert_eq!(fixed, r#"{"hello":"world"}"#);
            }
            _ => {
                panic!("Expected fixed body");
            }
        }
    }

    #[test]
    fn response_new_redirect_struct() {
        let location = "http://example.com";
        let response = Redirect::new(location).into_response().unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get(http::header::LOCATION).unwrap(),
            location
        );
    }

    #[test]
    fn redirect_status_codes() {
        let location = "http://example.com";

        for (redirect, expected) in [
            (Redirect::see_other(location), StatusCode::SEE_OTHER),
            (
                Redirect::permanent(location),
                StatusCode::PERMANENT_REDIRECT,
            ),
            (
                Redirect::temporary(location),
                StatusCode::TEMPORARY_REDIRECT,
            ),
            (
                Redirect::with_status_code(location, StatusCode::MOVED_PERMANENTLY),
                StatusCode::MOVED_PERMANENTLY,
            ),
        ] {
            assert_eq!(redirect.location(), location);
            assert_eq!(redirect.status_code(), expected);

            let response = redirect.into_response().unwrap();
            assert_eq!(response.status(), expected);
            assert_eq!(
                response.headers().get(http::header::LOCATION).unwrap(),
                location
            );
        }
    }

    #[test]
    #[should_panic(expected = "is not a redirection status code")]
    fn redirect_with_non_redirection_status_code() {
        let _ = Redirect::with_status_code("http://example.com", StatusCode::OK);
    }
}
