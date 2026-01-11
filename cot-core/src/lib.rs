mod body;

pub mod error;
pub mod headers;
pub mod html;
#[cfg(feature = "json")]
pub mod json;
pub mod response;

pub use body::{Body, BodyInner};
pub use error::Error;

/// A type alias for an HTTP status code.
pub type StatusCode = http::StatusCode;

/// A type alias for an HTTP method.
pub type Method = http::Method;

/// A type alias for a result that can return a [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
