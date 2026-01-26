//! Error handling types and utilities for Cot applications.
//!
//! This module provides error types, error handlers, and utilities for
//! handling various types of errors that can occur in Cot applications,
//! including 404 Not Found errors, uncaught panics, and custom error pages.

pub mod backtrace;
pub(crate) mod error_impl;
mod method_not_allowed;
mod uncaught_panic;

pub use error_impl::{Error, impl_into_cot_error};
pub use method_not_allowed::MethodNotAllowed;
pub use uncaught_panic::UncaughtPanic;
