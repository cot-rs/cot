pub mod backtrace;
pub(crate) mod error_impl;
mod method_not_allowed;
mod not_found;
mod uncaught_panic;

pub use error_impl::{Error, impl_into_cot_error};
pub use method_not_allowed::MethodNotAllowed;
pub use not_found::{Kind as NotFoundKind, NotFound};
pub use uncaught_panic::UncaughtPanic;
