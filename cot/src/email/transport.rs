use std::pin::Pin;

use cot::email::MessageBuildError;
use thiserror::Error;

use crate::email::EmailMessage;

pub mod console;
pub mod smtp;

const ERROR_PREFIX: &str = "email transport error:";

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransportError {
    #[error("{ERROR_PREFIX} transport error: {0}")]
    Transport(String),
    #[error("{ERROR_PREFIX} message build error: {0}")]
    MessageBuildError(#[from] MessageBuildError),
}

pub type TransportResult<T> = Result<T, TransportError>;

pub trait Transport: Send + Sync + 'static {
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
