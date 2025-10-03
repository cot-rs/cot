//! Re-exports of some of the Cot dependencies that are used in the macros
//! and the CLI.
//!
//! This is to avoid the need to add them as dependencies to the crate that uses
//! the macros.
//!
//! This is not a public API and should not be used directly.

#[cfg(feature = "openapi")]
pub use aide::openapi::{Operation, RequestBody, Response as OpenApiResponse, StatusCode};
/// Askama's macros don't work when Askama is re-exported, so there's no point
/// in re-exporting it publicly. However, we need to re-export it here so that
/// our macros can implement traits from Askama.
pub use askama;
pub use async_trait::async_trait;
pub use bytes::Bytes;
pub use cot_macros::ModelHelper;
pub use {indexmap, tokio};

// used in the CLI
#[cfg(feature = "db")]
pub use crate::utils::graph::apply_permutation;

/// The version of the crate.
///
/// This is used in the CLI to specify the version of the crate to use in the
/// `Cargo.toml` file when creating a new Cot project.
pub const COT_VERSION: &str = env!("CARGO_PKG_VERSION");
