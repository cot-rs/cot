use cot::email::EmailMessage;
use lettre::Message;

use crate::email::transport::Transport;

#[derive(Debug, Clone)]
pub struct Console;

impl Console {
    pub fn new() -> Self {
        Self {}
    }
}

impl Transport for Console {
    async fn send(&self, messages: &[EmailMessage]) -> Result<(), String> {
        for message in messages {
            let m: Message = message.clone().into();
            println!("{m:?}");
        }
        Ok(())
    }
}
