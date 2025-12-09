//! SMTP transport implementation.
//!
//! This backend uses the `lettre` crate to send messages to a remote SMTP
//! server. Credentials, server host and authentication mechanism are
//! configurable.
//!
//! Typical usage is through the high-level [`crate::email::Email`] API:
//!
//! ```no_run
//! use cot::common_types::Password;
//! use cot::email::transport::smtp::{Mechanism, SMTP, SMTPCredentials, SMTPServer};
//! use cot::email::{Email, EmailMessage};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let creds = SMTPCredentials::new("user@example.com", Password::from("secret"));
//! let smtp = SMTP::new(creds, SMTPServer::Gmail, Mechanism::Plain);
//! let email = Email::new(smtp);
//! let msg = EmailMessage::builder()
//!     .from("user@example.com".into())
//!     .to(vec!["user2@example.com".into()])
//!     .body("This is a test email.".into())
//!     .build()?;
//! email.send(msg).await?;
//! # Ok(()) }
//! ```
use cot::email::EmailMessage;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common_types::Password;
use crate::email::transport::{Transport, TransportError, TransportResult};

const ERROR_PREFIX: &str = "smtp transport error:";

/// Errors produced by the SMTP transport.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SMTPError {
    ///  An IO error occurred.
    #[error("{ERROR_PREFIX} IO error: {0}")]
    Io(#[from] std::io::Error),
    /// An error occurred while sending the email via SMTP.
    #[error("{ERROR_PREFIX} send error: {0}")]
    SmtpSend(Box<dyn std::error::Error + Send + Sync>),
}

impl From<SMTPError> for TransportError {
    fn from(err: SMTPError) -> Self {
        TransportError::Backend(err.to_string())
    }
}

/// Supported SMTP authentication mechanisms.
///
/// The default is `Plain`.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mechanism {
    /// PLAIN authentication mechanism defined in [RFC 4616](https://tools.ietf.org/html/rfc4616)
    /// This is the default authentication mechanism.
    #[default]
    Plain,
    /// LOGIN authentication mechanism defined in
    /// [draft-murchison-sasl-login-00](https://www.ietf.org/archive/id/draft-murchison-sasl-login-00.txt).
    Login,
    /// Non-standard XOAUTH2 mechanism defined in
    /// [xoauth2-protocol](https://developers.google.com/gmail/imap/xoauth2-protocol)
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

/// Credentials used to authenticate to an SMTP server.
#[derive(Debug, Clone)]
pub struct SMTPCredentials {
    auth_id: String,
    secret: Password,
}

impl SMTPCredentials {
    /// Create a new set of credentials.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Password;
    /// use cot::email::transport::smtp::SMTPCredentials;
    ///
    /// let creds = SMTPCredentials::new("testuser", Password::from("secret"));
    /// ```
    pub fn new<S: Into<String>>(username: S, password: Password) -> Self {
        Self {
            auth_id: username.into(),
            secret: password,
        }
    }
}

impl From<SMTPCredentials> for Credentials {
    fn from(credentials: SMTPCredentials) -> Self {
        Credentials::new(credentials.auth_id, credentials.secret.into_string())
    }
}

/// The SMTP host/server to connect to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SMTPServer {
    /// Google's SMTP server.
    Gmail,
    /// Localhost SMTP server.
    Localhost,
}

impl SMTPServer {
    /// Returns the hostname for the server.
    pub fn as_str(&self) -> &str {
        match self {
            SMTPServer::Gmail => "smtp.gmail.com",
            SMTPServer::Localhost => "localhost",
        }
    }
}

/// SMTP transport backend that sends emails via a remote SMTP server.
///
/// # Examples
///
/// ```no_run
/// use cot::email::{Email, EmailMessage};
/// use cot::email::transport::smtp::{SMTP, SMTPCredentials, SMTPServer, Mechanism};
/// use cot::common_types::Password;
///
/// # async fn run() -> cot::Result<()> {
/// let creds = SMTPCredentials::new("username", Password::from("password"));
/// let smtp = SMTP::new(creds, SMTPServer::Gmail, Mechanism::Plain);
/// let email = Email::new(smtp);
/// let recipients = vec!["testreceipient@example.com".into()];
/// let msg = EmailMessage::builder()
///  .from("testfrom@example.com".into())
/// .to(recipients)
/// .body("This is a test email.".into())
/// .build()?;
/// email.send(msg).await?;
/// # Ok(()) }
#[derive(Debug, Clone)]
pub struct SMTP {
    credentials: SMTPCredentials,
    host: SMTPServer,
    mechanism: Mechanism,
}

impl SMTP {
    /// Create a new SMTP transport backend.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Password;
    /// use cot::email::transport::smtp::{Mechanism, SMTP, SMTPCredentials, SMTPServer};
    ///
    /// let creds = SMTPCredentials::new("username", Password::from("password"));
    /// let smtp = SMTP::new(creds, SMTPServer::Gmail, Mechanism::Plain);
    /// ```
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
