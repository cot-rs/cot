//! Error types and utilities for handling uncaught panics.

use std::any::Any;
use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard};

use thiserror::Error;

use crate::error::error_impl::impl_into_cot_error;

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
/// use cot::error::UncaughtPanic;
///
/// // This would typically be created internally by Cot when catching panics
/// let panic = UncaughtPanic::new(Box::new("something went wrong"));
/// ```
#[derive(Debug, Clone, Error)]
#[error("an unexpected error occurred")]
pub struct UncaughtPanic {
    payload: Arc<Mutex<Box<dyn Any + Send + 'static>>>,
}
impl_into_cot_error!(UncaughtPanic, INTERNAL_SERVER_ERROR);

impl UncaughtPanic {
    /// Creates a new `UncaughtPanic` with the given panic payload.
    ///
    /// This method is typically used internally by Cot when catching panics
    /// that occur during request processing.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::error::UncaughtPanic;
    ///
    /// let panic = UncaughtPanic::new(Box::new("a panic occurred"));
    /// ```
    #[must_use]
    pub fn new(payload: Box<dyn Any + Send + 'static>) -> Self {
        Self {
            payload: Arc::new(Mutex::new(payload)),
        }
    }

    /// Returns a wrapper over the panic payload.
    ///
    /// This method provides access to the original panic payload, which can be
    /// useful for debugging purposes.
    ///
    /// # Panics
    ///
    /// This method will panic if the internal mutex cannot be locked.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::error::UncaughtPanic;
    ///
    /// let panic = UncaughtPanic::new(Box::new("test panic"));
    /// let payload = panic.payload();
    /// ```
    #[must_use]
    pub fn payload(&self) -> UncaughtPanicPayload<'_> {
        let mutex_guard = self.payload.lock().expect("failed to lock panic payload");
        UncaughtPanicPayload { mutex_guard }
    }
}

/// A wrapper around the panic payload that provides access to the original
/// panic data.
#[derive(Debug)]
pub struct UncaughtPanicPayload<'a> {
    mutex_guard: MutexGuard<'a, Box<dyn Any + Send + 'static>>,
}

impl Deref for UncaughtPanicPayload<'_> {
    type Target = Box<dyn Any + Send + 'static>;

    fn deref(&self) -> &Self::Target {
        &self.mutex_guard
    }
}
