//! Error handling types and utilities for Cot applications.
//!
//! This module provides error types, error handlers, and utilities for
//! handling various types of errors that can occur in Cot applications,
//! including 404 Not Found errors, uncaught panics, and custom error pages.

pub mod handler;

#[doc(inline)]
pub use cot_core::error::{MethodNotAllowed, NotFound, NotFoundKind, UncaughtPanic};
#[doc(inline)]
pub(crate) use cot_core::error::{backtrace, impl_into_cot_error};
