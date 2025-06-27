use std::any::Any;
use std::sync::{Arc, Mutex};

use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error("Uncaught panic occurred")]
pub struct UncaughtPanic {
    payload: Arc<Mutex<Box<dyn Any + Send + 'static>>>,
}

impl UncaughtPanic {
    #[must_use]
    pub fn new(payload: Box<dyn Any + Send + 'static>) -> Self {
        Self {
            payload: Arc::new(Mutex::new(payload)),
        }
    }

    #[must_use]
    pub fn payload(&self) -> Arc<Mutex<Box<dyn Any + Send + 'static>>> {
        Arc::clone(&self.payload)
    }
}
