pub mod handler;

#[doc(inline)]
pub use cot_core::error::{MethodNotAllowed, NotFound, NotFoundKind, UncaughtPanic};
#[doc(inline)]
pub(crate) use cot_core::error::{backtrace, impl_into_cot_error};
