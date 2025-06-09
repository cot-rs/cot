use thiserror::Error;

use crate::Method;

#[non_exhaustive]
#[derive(Debug, Error)]
#[error("Method `{method}` not allowed for this endpoint")]
pub struct MethodNotAllowed {
    method: Method,
}

impl MethodNotAllowed {
    pub fn new(method: Method) -> Self {
        Self { method }
    }

    pub fn method(&self) -> &Method {
        &self.method
    }
}
