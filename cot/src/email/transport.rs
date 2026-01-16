//! This module defines the email transport system for sending emails in Cot.
//!
//! It provides a [`Transport`] trait that can be implemented by different email
//! backends (e.g., SMTP, console). The module also defines error handling for
//! transport operations.
use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;

use cot::email::EmailMessageError;
use thiserror::Error;

use crate::email::EmailMessage;
use crate::error::error_impl::impl_into_cot_error;

pub mod console;
pub mod smtp;

const ERROR_PREFIX: &str = "email transport error:";

/// Errors that can occur while sending an email using a transport backend.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransportError {
    /// The underlying transport backend returned an error.
    #[error("{ERROR_PREFIX} transport error: {0}")]
    Backend(Box<dyn StdError + Send + Sync + 'static>),
    /// Failed to build the email message.
    #[error("{ERROR_PREFIX} message build error: {0}")]
    MessageBuildError(#[from] EmailMessageError),
}

impl_into_cot_error!(TransportError);

/// A Convenience alias for results returned by transport operations.
pub type TransportResult<T> = Result<T, TransportError>;

/// A generic asynchronous email transport interface.
///
/// The [`Transport`] trait abstracts over different email transport backends.
/// It provides methods to manage sending email messages asynchronously.
pub trait Transport: Send + Sync + 'static {
    /// Send one or more email messages.
    ///
    /// # Errors
    ///
    /// This method can return an error if there is an issue sending the
    /// messages.
    fn send(&self, messages: &[EmailMessage]) -> impl Future<Output = TransportResult<()>> + Send;
}

pub(crate) trait BoxedTransport: Send + Sync + 'static {
    fn send<'a>(
        &'a self,
        messages: &'a [EmailMessage],
    ) -> Pin<Box<dyn Future<Output = TransportResult<()>> + Send + 'a>>;
}

impl<T: Transport> BoxedTransport for T {
    fn send<'a>(
        &'a self,
        messages: &'a [EmailMessage],
    ) -> Pin<Box<dyn Future<Output = TransportResult<()>> + Send + 'a>> {
        Box::pin(async move { T::send(self, messages).await })
    }
}
