//! Error types and utilities for handling "404 Not Found" errors.

use thiserror::Error;

use crate::error::error_impl::impl_into_cot_error;

#[expect(clippy::doc_link_with_quotes, reason = "404 Not Found link")]
/// A ["404 Not Found"] error that can be returned by Cot applications.
///
/// This struct represents a "404 Not Found" error and can be used to indicate
/// that a requested resource was not found. It contains information about the
/// type of not-found error that occurred.
///
/// # Examples
///
/// ```
/// use cot::error::NotFound;
///
/// // Create a basic 404 error
/// let error = NotFound::new();
///
/// // Create a 404 error with a custom message
/// let error = NotFound::with_message("User not found");
/// ```
///
/// ["404 Not Found"]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status/404
#[derive(Debug, Error)]
#[non_exhaustive]
#[error(transparent)]
pub struct NotFound {
    /// The specific type of not-found error that occurred.
    pub kind: Kind,
}
impl_into_cot_error!(NotFound, NOT_FOUND);

impl NotFound {
    /// Creates a new `NotFound` error with a generic "Not Found" message.
    ///
    /// This is the most common way to create a 404 error when you don't need
    /// to provide additional context about what was not found.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::error::NotFound;
    ///
    /// let error = NotFound::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::with_kind(Kind::Custom)
    }

    /// Creates a new `NotFound` error with a custom message.
    ///
    /// This method allows you to provide additional context about what was
    /// not found, which can be useful for debugging or providing more
    /// informative error messages.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::error::NotFound;
    ///
    /// let error = NotFound::with_message("User with ID 123 not found");
    /// let page_name = "home";
    /// let error = NotFound::with_message(format!("Page '{}' not found", page_name));
    /// ```
    #[must_use]
    pub fn with_message<T: Into<String>>(message: T) -> Self {
        Self::with_kind(Kind::WithMessage(message.into()))
    }

    #[must_use]
    pub(crate) fn router() -> Self {
        Self::with_kind(Kind::FromRouter)
    }

    #[must_use]
    fn with_kind(kind: Kind) -> Self {
        NotFound { kind }
    }
}

impl Default for NotFound {
    fn default() -> Self {
        Self::new()
    }
}

/// The specific type of not-found error that occurred.
///
/// This enum provides different variants for different types of 404 errors,
/// allowing for more specific error handling and messaging.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Kind {
    /// A 404 error that originated from the router when no matching route was
    /// found.
    ///
    /// This variant is used when the router cannot find a route that matches
    /// the request's path and method.
    #[error("Not Found")]
    #[non_exhaustive]
    FromRouter,
    /// A generic 404 error without additional context.
    ///
    /// This variant is used for basic "not found" errors where no specific
    /// message or context is needed.
    #[error("Not Found")]
    #[non_exhaustive]
    Custom,
    /// A 404 error with a custom message providing additional context.
    ///
    /// This variant includes a custom message that describes what specifically
    /// was not found, which can be useful for debugging or providing more
    /// informative error responses.
    #[error("Not Found: {0}")]
    #[non_exhaustive]
    WithMessage(String),
}
