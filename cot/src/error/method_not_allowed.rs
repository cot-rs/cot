use thiserror::Error;

use crate::Method;
use crate::error::error_impl::impl_into_cot_error;

#[non_exhaustive]
#[derive(Debug, Error)]
#[error("method `{method}` not allowed for this endpoint")]
pub struct MethodNotAllowed {
    method: Method,
}
impl_into_cot_error!(MethodNotAllowed, METHOD_NOT_ALLOWED);

impl MethodNotAllowed {
    #[must_use]
    pub fn new(method: Method) -> Self {
        Self { method }
    }

    #[must_use]
    pub fn method(&self) -> &Method {
        &self.method
    }
}
