//! HTTP response type and helper methods.
//!
//! Cot uses the [`Response`](http::Response) type from the [`http`] crate
//! to represent outgoing HTTP responses. However, it also provides a
//! [`ResponseExt`] trait that contain various helper methods for working with
//! HTTP responses. These methods are used to create new responses with HTML
//! content types, redirects, and more. You probably want to have a `use`
//! statement for [`ResponseExt`] in your code most of the time to be able to
//! use these functions:
//!
//! ```
//! use cot::response::ResponseExt;
//! ```

use crate::{Body, StatusCode};

mod into_response;

/// Derive macro for the [`IntoResponse`] trait.
///
/// This macro can be applied to enums to automatically implement the
/// [`IntoResponse`] trait. The enum must consist of tuple variants with
/// exactly one field each, where each field type implements [`IntoResponse`].
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
/// working with HTTP response.
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

    /// Create a new redirect response.
    ///
    /// This creates a new [`Response`] object with a status code of
    /// [`StatusCode::SEE_OTHER`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status/303)
    /// and a location header set to the provided location.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::StatusCode;
    /// use cot::response::{Response, ResponseExt};
    ///
    /// let response = Response::new_redirect("http://example.com");
    /// ```
    ///
    /// # See also
    ///
    /// * [`crate::reverse_redirect!`] – a more ergonomic way to create
    ///   redirects to internal views
    #[must_use]
    #[deprecated(since = "0.5.0", note = "Use Redirect::new() instead")]
    fn new_redirect<T: Into<String>>(location: T) -> Self;
}

impl private::Sealed for Response {}

impl ResponseExt for Response {
    fn builder() -> http::response::Builder {
        http::Response::builder()
    }

    fn new_redirect<T: Into<String>>(location: T) -> Self {
        http::Response::builder()
            .status(StatusCode::SEE_OTHER)
            .header(http::header::LOCATION, location.into())
            .body(Body::empty())
            .expect(RESPONSE_BUILD_FAILURE)
    }
}

/// A redirect response.
///
/// This type creates an HTTP redirect response with a status code of
/// [`StatusCode::SEE_OTHER`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status/303)
/// (303) and a `Location` header set to the specified URL.
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
pub struct Redirect(String);

impl Redirect {
    /// Creates a new redirect response to the specified location.
    ///
    /// Creates an HTTP redirect response with a status code of
    /// [`StatusCode::SEE_OTHER`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status/303)
    /// (303) and a `Location` header set to the specified URL.
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
        Self(location.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::BodyInner;
    use crate::headers::JSON_CONTENT_TYPE;
    use crate::response::{Response, ResponseExt};

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
    #[expect(deprecated)]
    fn response_new_redirect() {
        let location = "http://example.com";
        let response = Response::new_redirect(location);
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get(http::header::LOCATION).unwrap(),
            location
        );
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
}
