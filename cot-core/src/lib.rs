pub use crate::error::error_impl::Error;

pub mod body;
/// Error handling types and utilities for Cot applications.
///
/// This module provides error types, error handlers, and utilities for
/// handling various types of errors that can occur in Cot applications,
/// including 404 Not Found errors, uncaught panics, and custom error pages.
pub mod error;
pub mod headers;
pub mod request;
pub mod response;
#[macro_use]
pub mod handler;
pub mod html;
pub mod middleware;
pub mod openapi;
pub mod router;

/// A type alias for a result that can return a [`cot_core::Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// A type alias for an HTTP status code.
pub type StatusCode = http::StatusCode;

/// A type alias for an HTTP method.
pub type Method = http::Method;

pub use crate::body::Body;
