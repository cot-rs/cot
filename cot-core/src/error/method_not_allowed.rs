//! Error types and utilities for handling "405 Method Not Allowed" errors.

use thiserror::Error;

use crate::Method;
use crate::error::error_impl::impl_into_cot_error;

#[expect(clippy::doc_link_with_quotes, reason = "405 Method Not Allowed link")]
/// A ["405 Method Not Allowed"] error that can be returned by Cot applications.
///
/// # Examples
///
/// ```
/// use cot::error::MethodNotAllowed;
///
/// let error = MethodNotAllowed::new(cot::Method::POST);
/// assert_eq!(error.method, &cot::Method::POST);
/// ```
///
/// ["405 Method Not Allowed"]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status/405
#[non_exhaustive]
#[derive(Debug, Error)]
#[error("method `{method}` not allowed for this endpoint")]
pub struct MethodNotAllowed {
    /// The HTTP method that was not allowed.
    pub method: Method,
}
impl_into_cot_error!(MethodNotAllowed, METHOD_NOT_ALLOWED);

impl MethodNotAllowed {
    /// Creates a new `MethodNotAllowed` error with the specified HTTP method.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::error::MethodNotAllowed;
    ///
    /// let error = MethodNotAllowed::new(cot::Method::POST);
    /// assert_eq!(error.method, cot::Method::POST);
    /// ```
    #[must_use]
    pub fn new(method: Method) -> Self {
        Self { method }
    }
}
