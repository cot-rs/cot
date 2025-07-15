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
    #[must_use]
    pub fn new<E>(error: E) -> Self
    where
        E: Into<Box<dyn StdError + Send + Sync + 'static>>,
    {
        Self::with_status(error, StatusCode::INTERNAL_SERVER_ERROR)
    }

    /// Create a new error with a custom error message or error type.
    ///
    /// The error will be associated with a 500 Internal Server Error
    /// status code, which is the default for unexpected errors.
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
        Self {
            inner: Box::new(ErrorImpl {
                inner: error.into(),
                status_code: Some(status_code),
                backtrace: __cot_create_backtrace(),
            }),
        }
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
        note = "Use `cot::Error::new` or `cot::Error::with_status` directly instead",
        since = "0.4.0"
    )]
    pub fn admin<E>(error: E) -> Self
    where
        E: Into<Box<dyn StdError + Send + Sync + 'static>>,
    {
        Self::new(error)
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

    /// If the error is a custom error, returns a reference to the inner
    /// `cot::Error`, if any (recursively). If the error is not a custom
    /// error, returns a reference to itself.
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
#[error("Failed to render template: {0}")]
struct TemplateRender(#[from] askama::Error);
impl_into_cot_error!(TemplateRender);
impl From<askama::Error> for Error {
    fn from(err: askama::Error) -> Self {
        Error::from(TemplateRender(err))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Error while accessing the session object")]
struct SessionAccess(#[from] tower_sessions::session::Error);
impl_into_cot_error!(SessionAccess);
impl From<tower_sessions::session::Error> for Error {
    fn from(err: tower_sessions::session::Error) -> Self {
        Error::from(SessionAccess(err))
    }
}

#[cfg(test)]
mod tests {

    // #[test]
    // fn test_error_new() {
    //     let inner = ErrorKind::StartServer {
    //         source: io::Error::other("server error"),
    //     };
    //
    //     let error = Error::from_kind(inner);
    //
    //     assert!(StdError::source(&error).is_some());
    // }
    //
    // #[test]
    // fn test_error_display() {
    //     let inner = ErrorKind::InvalidContentType {
    //         expected: "application/json",
    //         actual: "text/html".to_string(),
    //     };
    //     let error = Error::from_kind(inner);
    //
    //     let display = format!("{error}");
    //
    //     assert_eq!(
    //         display,
    //         "Invalid content type; expected `application/json`, found
    // `text/html`"     );
    // }
    //
    // #[test]
    // fn test_error_from_repr() {
    //     let inner = ErrorKind::NoViewToReverse {
    //         app_name: None,
    //         view_name: "home".to_string(),
    //     };
    //
    //     let error: Error = inner.into();
    //
    //     assert_eq!(
    //         format!("{error}"),
    //         "Failed to reverse route `home` due to view not existing"
    //     );
    // }
}
