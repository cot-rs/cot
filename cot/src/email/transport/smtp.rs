use cot::email::EmailMessage;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use serde::{Deserialize, Serialize};

use crate::common_types::Password;
use crate::email::transport::Transport;

#[derive(Debug, Clone)]
pub struct SMTPCredentials {
    username: String,
    password: Password,
}

impl From<SMTPCredentials> for Credentials {
    fn from(credentials: SMTPCredentials) -> Self {
        Credentials::new(credentials.username, credentials.password.into_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SMTPHost {
    Gmail,
    Localhost,
}

impl SMTPHost {
    pub fn as_str(&self) -> &str {
        match self {
            SMTPHost::Gmail => "smtp.gmail.com",
            SMTPHost::Localhost => "localhost",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SMTP {
    credentials: SMTPCredentials,
    host: SMTPHost,
}

impl SMTP {
    pub fn new(credentials: SMTPCredentials, host: SMTPHost) -> Self {
        Self { credentials, host }
    }
}

impl Transport for SMTP {
    async fn send(&self, messages: &[EmailMessage]) -> Result<(), String> {
        for message in messages {
            let mailer = AsyncSmtpTransport::relay(self.host.as_str())
                .unwrap()
                .credentials(self.credentials.clone().into())
                .build::<Tokio1Executor>();
            mailer.send(message.clone().into()).await.unwrap();
        }

        Ok(())
    }
}
