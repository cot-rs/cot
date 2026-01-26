//! JSON rendering utilities.
//!
//! This module provides structures and methods for creating and rendering JSON
//! content.

/// A type that represents JSON content.
///
/// Note that this is just a newtype wrapper around data and does not provide
/// any content validation. It is primarily useful as a request extractor and
/// response type for RESTful endpoints.
///
/// # Examples
///
/// ```
/// use cot::json::Json;
///
/// let Json(data) = Json("content");
/// assert_eq!(data, "content");
/// ```
#[cfg(feature = "json")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Json<D>(pub D);
