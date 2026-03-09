use std::error::Error as StdError;
use std::fmt::Display;
use std::ops::Deref;

use derive_more::with_trait::Debug;

use crate::StatusCode;
// Need to rename Backtrace to CotBacktrace, because otherwise it triggers special behavior
// in the thiserror library
use crate::error::backtrace::{__cot_create_backtrace, Backtrace as CotBacktrace};

/// An error that can occur while using Cot.
pub struct Error {
    repr: Box<ErrorImpl>,
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
            repr: Box::new(ErrorImpl {
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
            repr: Box::new(ErrorImpl {
                inner: error.into(),
                status_code: Some(status_code),
                backtrace: __cot_create_backtrace(),
            }),
        };
        Self::wrap(WithStatusCode(error))
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
            .repr
            .status_code
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }

    #[must_use]
    #[doc(hidden)]
    pub fn backtrace(&self) -> &CotBacktrace {
        &self.repr.backtrace
    }

    /// Returns a reference to inner `Error`, if `self` is wrapping a wrapper.
    /// Otherwise, it returns `self`.
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
            if let Some(error) = inner.downcast_ref::<Self>()
                && !error.is_wrapper()
            {
                return error;
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
        self.repr.status_code.is_none()
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.repr, f)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.repr.inner, f)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.repr.inner.source()
    }
}

impl Deref for Error {
    type Target = dyn StdError + Send + Sync;

    fn deref(&self) -> &Self::Target {
        &*self.repr.inner
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

#[macro_export]
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
pub use impl_into_cot_error;

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
    use serde::ser::Error as _;

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

    #[test]
    fn error_wrap_and_is_wrapper() {
        let inner = std::io::Error::other("wrapped");
        let error = Error::wrap(inner);

        assert!(error.is_wrapper());
        assert_eq!(error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn error_with_status_propagation() {
        let error = Error::with_status("bad request", StatusCode::BAD_REQUEST);
        assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);
        // wrapping again should not override the status code
        let wrapped = Error::wrap(error);

        assert_eq!(wrapped.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn error_inner_returns_original() {
        let error = Error::with_status("bad request", StatusCode::BAD_REQUEST);
        let wrapped = Error::wrap(error);

        assert_eq!(wrapped.inner().status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn error_inner_multiple_wrapped() {
        let error = Error::with_status("bad request", StatusCode::BAD_REQUEST);
        let wrapped = Error::wrap(error);
        let wrapped_twice = Error::wrap(wrapped);
        let wrapped_thrice = Error::wrap(wrapped_twice);

        assert_eq!(wrapped_thrice.to_string(), "bad request");
        assert!(wrapped_thrice.source().is_some());
        assert!(wrapped_thrice.source().unwrap().source().is_none());
        assert_eq!(
            wrapped_thrice.inner().status_code(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn error_deref_to_inner() {
        let error = Error::internal("deref test");
        let msg = format!("{}", &*error);

        assert_eq!(msg, "deref test");
    }

    #[test]
    fn error_from_template_render() {
        let askama_err = askama::Error::Custom(Box::new(std::io::Error::other("fail")));
        let error: Error = askama_err.into();

        assert!(error.to_string().contains("failed to render template"));
    }

    #[test]
    fn error_from_session_access() {
        let session_err =
            tower_sessions::session::Error::SerdeJson(serde_json::Error::custom("session error"));

        let error: Error = session_err.into();

        assert!(
            error
                .to_string()
                .contains("error while accessing the session object")
        );
    }
}
