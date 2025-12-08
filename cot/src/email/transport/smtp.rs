use cot::email::EmailMessage;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common_types::Password;
use crate::email::transport::{Transport, TransportError, TransportResult};

const ERROR_PREFIX: &str = "smtp transport error:";

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SMTPError {
    #[error("{ERROR_PREFIX} IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{ERROR_PREFIX} send error: {0}")]
    SmtpSend(Box<dyn std::error::Error + Send + Sync>),
}

impl From<SMTPError> for TransportError {
    fn from(err: SMTPError) -> Self {
        TransportError::Transport(err.to_string())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
#[serde(rename_all = "lowercase")]
pub enum SMTPServer {
    Gmail,
    Localhost,
}

impl SMTPServer {
    pub fn as_str(&self) -> &str {
        match self {
            SMTPServer::Gmail => "smtp.gmail.com",
            SMTPServer::Localhost => "localhost",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SMTP {
    credentials: SMTPCredentials,
    host: SMTPServer,
    mechanism: Mechanism,
}

impl SMTP {
    pub fn new(credentials: SMTPCredentials, host: SMTPServer, mechanism: Mechanism) -> Self {
        Self {
            credentials,
            host,
            mechanism,
        }
    }
}

impl Transport for SMTP {
    async fn send(&self, messages: &[EmailMessage]) -> TransportResult<()> {
        let mechanisms: Vec<lettre::transport::smtp::authentication::Mechanism> =
            vec![self.mechanism.clone().into()];
        for message in messages {
            let m = Message::try_from(message.clone())?;
            let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(self.host.as_str())
                .map_err(|err| SMTPError::SmtpSend(Box::new(err)))?
                .credentials(self.credentials.clone().into())
                .authentication(mechanisms.clone())
                .build();
            mailer
                .send(m)
                .await
                .map_err(|err| SMTPError::SmtpSend(Box::new(err)))?;
        }
        Ok(())
    }
}
