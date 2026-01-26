//! Core types and functionality for the Cot web framework.
//!
//! This crate provides the foundational building blocks for
//! [Cot](https://docs.rs/cot/latest/cot/), including HTTP primitives, body handling, error
//! types, handlers, middleware, and request/response types.
//!
//! Most applications should use the main `cot` crate rather than depending on
//! `cot-core` directly. This crate is primarily intended for internal use by
//! the Cot framework and for building custom extensions.

mod body;

pub mod error;
#[macro_use]
pub mod handler;
pub mod headers;
pub mod html;
#[cfg(feature = "json")]
pub mod json;
pub mod middleware;
pub mod request;
pub mod response;

pub use body::Body;
pub use error::Error;

/// A type alias for an HTTP status code.
pub type StatusCode = http::StatusCode;

/// A type alias for an HTTP method.
pub type Method = http::Method;

/// A type alias for a result that can return a [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
