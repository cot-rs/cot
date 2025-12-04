use cot::email::EmailMessage;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use serde::{Deserialize, Serialize};

use crate::common_types::Password;
use crate::email::transport::Transport;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mechanism {
    #[default]
    Plain,
    Login,
    Xoauth2,
}

impl From<Mechanism> for lettre::transport::smtp::authentication::Mechanism {
    fn from(mechanism: Mechanism) -> Self {
        match mechanism {
            Mechanism::Plain => lettre::transport::smtp::authentication::Mechanism::Plain,
            Mechanism::Login => lettre::transport::smtp::authentication::Mechanism::Login,
            Mechanism::Xoauth2 => lettre::transport::smtp::authentication::Mechanism::Xoauth2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SMTPCredentials {
    username: String,
    password: Password,
}

impl SMTPCredentials {
    pub fn new(username: String, password: Password) -> Self {
        Self { username, password }
    }
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
    mechanism: Mechanism,
}

impl SMTP {
    pub fn new(credentials: SMTPCredentials, host: SMTPHost, mechanism: Mechanism) -> Self {
        Self {
            credentials,
            host,
            mechanism,
        }
    }
}

impl Transport for SMTP {
    async fn send(&self, messages: &[EmailMessage]) -> Result<(), String> {
        let mechanisms: Vec<lettre::transport::smtp::authentication::Mechanism> =
            vec!(self.mechanism.clone().into());
        for message in messages {
            let mailer = AsyncSmtpTransport::relay(self.host.as_str())
                .unwrap()
                .credentials(self.credentials.clone().into())
                .authentication(mechanisms.clone())
                .build::<Tokio1Executor>();
            mailer.send(message.clone().into()).await.unwrap();
        }

        Ok(())
    }
}
