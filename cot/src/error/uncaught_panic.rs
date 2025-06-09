//! Error types and utilities for handling uncaught panics.

use std::any::Any;
use std::sync::{Arc, Mutex};

use thiserror::Error;

/// An error that represents an uncaught panic that occurred during request
/// processing.
///
/// This struct is used to wrap panics that occur in request handlers or other
/// async code, allowing them to be handled gracefully by Cot's error handling
/// system instead of crashing the entire application.
///
/// The panic payload is stored in a thread-safe manner and can be accessed
/// for debugging purposes, though it should be handled carefully as it may
/// contain sensitive information.
///
/// # Examples
///
/// ```
/// use cot::error::uncaught_panic::UncaughtPanic;
///
/// // This would typically be created internally by Cot when catching panics
/// let panic = UncaughtPanic::new(Box::new("Something went wrong"));
/// ```
#[derive(Debug, Clone, Error)]
#[error("Uncaught panic occurred")]
pub struct UncaughtPanic {
    payload: Arc<Mutex<Box<dyn Any + Send + 'static>>>,
}

impl UncaughtPanic {
    /// Creates a new `UncaughtPanic` with the given panic payload.
    ///
    /// This method is typically used internally by Cot when catching panics
    /// that occur during request processing.
    ///
    /// # Arguments
    ///
    /// * `payload` - The panic payload, which can be any type that implements
    ///   `Any + Send + 'static`
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::error::uncaught_panic::UncaughtPanic;
    ///
    /// let panic = UncaughtPanic::new(Box::new("A panic occurred"));
    /// ```
    #[must_use]
    pub fn new(payload: Box<dyn Any + Send + 'static>) -> Self {
        Self {
            payload: Arc::new(Mutex::new(payload)),
        }
    }

    /// Returns a reference to the panic payload.
    ///
    /// This method provides access to the original panic payload, which can be
    /// useful for debugging purposes. The payload is returned as a thread-safe
    /// reference that can be cloned and shared.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::error::uncaught_panic::UncaughtPanic;
    ///
    /// let panic = UncaughtPanic::new(Box::new("Test panic"));
    /// let payload = panic.payload();
    /// ```
    #[must_use]
    pub fn payload(&self) -> Arc<Mutex<Box<dyn Any + Send + 'static>>> {
        Arc::clone(&self.payload)
    }
}
