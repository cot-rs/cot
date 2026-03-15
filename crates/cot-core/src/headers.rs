//! HTTP header constants.
//!
//! This module provides commonly used content type header values.

pub const HTML_CONTENT_TYPE: &str = "text/html; charset=utf-8";
pub const MULTIPART_FORM_CONTENT_TYPE: &str = "multipart/form-data";
pub const URLENCODED_FORM_CONTENT_TYPE: &str = "application/x-www-form-urlencoded";
#[cfg(feature = "json")]
pub const JSON_CONTENT_TYPE: &str = "application/json";
pub const PLAIN_TEXT_CONTENT_TYPE: &str = "text/plain; charset=utf-8";
pub const OCTET_STREAM_CONTENT_TYPE: &str = "application/octet-stream";
