use std::error::Error as StdError;
use std::fmt::Display;
use std::ops::Deref;

use derive_more::with_trait::Debug;

use crate::StatusCode;
// Need to rename Backtrace to CotBacktrace, because otherwise it triggers special behavior
// in the thiserror library
use crate::error::backtrace::{__cot_create_backtrace, Backtrace as CotBacktrace};
use crate::error::not_found::NotFound;

/// An error that can occur while using Cot.
pub struct Error {
    inner: Box<ErrorImpl>,
}

impl Error {
    /// Create a new error with a custom error message or error type.
    ///
    /// This method is used to create a new error that does not have a specific
    /// HTTP status code associated with it. If in the chain of `Error` sources
    /// there is an error with a status code, it will be used instead. If not,
    /// the default status code of 500 Internal Server Error will be used.
    ///
    /// To get the first instance of `Error` in the chain that has a
    /// status code, use the [`Error::inner`] method.
    #[must_use]
    pub fn wrap<E>(error: E) -> Self
    where
        E: Into<Box<dyn StdError + Send + Sync + 'static>>,
    {
        Self {
            inner: Box::new(ErrorImpl {
                inner: error.into(),
                status_code: None,
                backtrace: __cot_create_backtrace(),
            }),
        }
    }

    /// Create a new error with a custom error message or error type.
    ///
    /// The error will be associated with a 500 Internal Server Error
    /// status code, which is the default for unexpected errors.
    ///
    /// If you want to create an error with a different status code,
    /// use [`Error::with_status`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Error;
    ///
    /// let error = Error::internal("An error occurred");
    /// let error = Error::internal(std::io::Error::new(
    ///     std::io::ErrorKind::Other,
    ///     "An error occurred",
    /// ));
    /// ```
    #[must_use]
    pub fn internal<E>(error: E) -> Self
    where
        E: Into<Box<dyn StdError + Send + Sync + 'static>>,
    {
        Self::with_status(error, StatusCode::INTERNAL_SERVER_ERROR)
    }

    /// Create a new error with a custom error message or error type and a
    /// specific HTTP status code.
    ///
    /// This method allows you to create an error with a custom status code,
    /// which will be returned in the HTTP response. This is useful when you
    /// want to return specific HTTP status codes like 400 Bad Request, 403
    /// Forbidden, etc.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::{Error, StatusCode};
    ///
    /// // Create a 400 Bad Request error
    /// let error = Error::with_status("Invalid input", StatusCode::BAD_REQUEST);
    ///
    /// // Create a 403 Forbidden error
    /// let error = Error::with_status("Access denied", StatusCode::FORBIDDEN);
    /// ```
    #[must_use]
    pub fn with_status<E>(error: E, status_code: StatusCode) -> Self
    where
        E: Into<Box<dyn StdError + Send + Sync + 'static>>,
    {
        let error = Self {
            inner: Box::new(ErrorImpl {
                inner: error.into(),
                status_code: Some(status_code),
                backtrace: __cot_create_backtrace(),
            }),
        };
        Self::wrap(WithStatusCode(error))
    }

    /// Create a new admin panel error with a custom error message or error
    /// type.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Error;
    ///
    /// let error = Error::admin("An error occurred");
    /// let error = Error::admin(std::io::Error::new(
    ///     std::io::ErrorKind::Other,
    ///     "An error occurred",
    /// ));
    /// ```
    #[deprecated(
        note = "Use `cot::Error::wrap`, `cot::Error::internal`, or \
        `cot::Error::with_status` directly instead",
        since = "0.4.0"
    )]
    pub fn admin<E>(error: E) -> Self
    where
        E: Into<Box<dyn StdError + Send + Sync + 'static>>,
    {
        Self::internal(error)
    }

    /// Create a new "404 Not Found" error without a message.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Error;
    ///
    /// let error = Error::not_found();
    /// ```
    #[must_use]
    #[deprecated(
        note = "Use `cot::Error::from(cot::error::not_found::NotFound::new())` instead",
        since = "0.4.0"
    )]
    pub fn not_found() -> Self {
        Self::from(NotFound::new())
    }

    /// Create a new "404 Not Found" error with a message.
    ///
    /// Note that the message is only displayed when Cot's debug mode is
    /// enabled. It will not be exposed to the user in production.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::Error;
    ///
    /// let id = 123;
    /// let error = Error::not_found_message(format!("User with id={id} not found"));
    /// ```
    #[must_use]
    #[deprecated(
        note = "Use `cot::Error::from(cot::error::not_found::NotFound::with_message())` instead",
        since = "0.4.0"
    )]
    pub fn not_found_message(message: String) -> Self {
        Self::from(NotFound::with_message(message))
    }

    /// Returns the HTTP status code associated with this error.
    ///
    /// This method returns the appropriate HTTP status code that should be
    /// sent in the response when this error occurs.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::{Error, StatusCode};
    ///
    /// let error = Error::internal("Something went wrong");
    /// assert_eq!(error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    ///
    /// let error = Error::with_status("Bad request", StatusCode::BAD_REQUEST);
    /// assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);
    /// ```
    #[must_use]
    pub fn status_code(&self) -> StatusCode {
        self.inner()
            .inner
            .status_code
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }

    #[must_use]
    pub(crate) fn backtrace(&self) -> &CotBacktrace {
        &self.inner.backtrace
    }

    /// Returns a reference to inner `Error`, if `self` is wrapping a wrapper.
    ///
    /// If this error is a wrapper around another `Error`, this method will
    /// return the inner `Error` that has a specific status code.
    ///
    /// This is useful for extracting the original error that caused the
    /// error, especially when dealing with errors that may have been
    /// wrapped multiple times in the error chain (e.g., by middleware or
    /// other error handling logic). You should use this method most
    /// of the time when you need to access the original error.
    ///
    /// # See also
    ///
    /// - [`Error::wrap`]
    #[must_use]
    pub fn inner(&self) -> &Self {
        let mut error: &dyn StdError = self;
        while let Some(inner) = error.source() {
            if let Some(error) = inner.downcast_ref::<Self>() {
                if !error.is_wrapper() {
                    return error;
                }
            }
            error = inner;
        }
        self
    }

    /// Returns `true` if this error is a wrapper around another error.
    ///
    /// In other words, this returns `true` if the error has been created
    /// with [`Error::wrap`], which means it does not have a specific
    /// HTTP status code associated with it. Otherwise, it returns `false`.
    #[must_use]
    pub fn is_wrapper(&self) -> bool {
        self.inner.status_code.is_none()
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner.inner, f)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner.inner.source()
    }
}

impl Deref for Error {
    type Target = dyn StdError + Send + Sync;

    fn deref(&self) -> &Self::Target {
        &*self.inner.inner
    }
}

#[derive(Debug)]
struct ErrorImpl {
    inner: Box<dyn StdError + Send + Sync>,
    status_code: Option<StatusCode>,
    #[debug(skip)]
    backtrace: CotBacktrace,
}

/// Indicates that the inner `Error` has a status code associated with it.
///
/// This is important, as we need to have this `Error` to be returned
/// by `std::error::Error::source` to be able to extract the status code.
#[derive(Debug)]
struct WithStatusCode(Error);

impl Display for WithStatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl StdError for WithStatusCode {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.0)
    }
}

impl From<Error> for askama::Error {
    fn from(value: Error) -> Self {
        askama::Error::Custom(Box::new(value))
    }
}

macro_rules! impl_into_cot_error {
    ($error_ty:ty) => {
        impl From<$error_ty> for $crate::Error {
            fn from(err: $error_ty) -> Self {
                $crate::Error::internal(err)
            }
        }
    };
    ($error_ty:ty, $status_code:ident) => {
        impl From<$error_ty> for $crate::Error {
            fn from(err: $error_ty) -> Self {
                $crate::Error::with_status(err, $crate::StatusCode::$status_code)
            }
        }
    };
}
pub(crate) use impl_into_cot_error;

#[derive(Debug, thiserror::Error)]
#[error("failed to render template: {0}")]
struct TemplateRender(#[from] askama::Error);
impl_into_cot_error!(TemplateRender);
impl From<askama::Error> for Error {
    fn from(err: askama::Error) -> Self {
        Error::from(TemplateRender(err))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("error while accessing the session object")]
struct SessionAccess(#[from] tower_sessions::session::Error);
impl_into_cot_error!(SessionAccess);
impl From<tower_sessions::session::Error> for Error {
    fn from(err: tower_sessions::session::Error) -> Self {
        Error::from(SessionAccess(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_new() {
        let inner = std::io::Error::other("server error");
        let error = Error::wrap(inner);

        assert!(StdError::source(&error).is_none());
        assert_eq!(error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn error_display() {
        let inner = std::io::Error::other("server error");
        let error = Error::internal(inner);

        let display = format!("{error}");

        assert_eq!(display, "server error");
    }
}
