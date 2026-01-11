pub mod error;
pub use error::Error;

/// A type alias for an HTTP status code.
pub type StatusCode = http::StatusCode;

/// A type alias for an HTTP method.
pub type Method = http::Method;
