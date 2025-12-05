use std::pin::Pin;

use crate::email::EmailMessage;

pub mod console;
pub mod smtp;

pub trait Transport: Send + Sync + 'static {
    fn send(&self, messages: &[EmailMessage]) -> impl Future<Output = Result<(), String>> + Send;
}

pub(crate) trait BoxedTransport: Send + Sync + 'static {
    fn send<'a>(
        &'a self,
        messages: &[EmailMessage],
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;
}

impl<T: Transport> BoxedTransport for T {
    fn send<'a>(
        &'a self,
        messages: &[EmailMessage],
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move { T::send(self, messages).await })
    }
}
