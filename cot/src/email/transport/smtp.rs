//! SMTP transport implementation.
//!
//! This backend uses the `lettre` crate to send messages to a remote SMTP
//! server. Credentials, server host and authentication mechanism are
//! configurable.
//!
//! Typical usage is through the high-level [`crate::email::Email`] API:
//!
//! ```no_run
//! use cot::common_types::Email;
//! use cot::config::EmailUrl;
//! use cot::email::EmailMessage;
//! use cot::email::transport::Transport;
//! use cot::email::transport::smtp::{Mechanism, Smtp};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let url = EmailUrl::from("smtps://username:password@smtp.gmail.com?tls=required");
//! let smtp = Smtp::new(&url, Mechanism::Plain)?;
//! let email = cot::email::Email::new(smtp);
//! let msg = EmailMessage::builder()
//!     .from(Email::try_from("user@example.com").unwrap())
//!     .to(vec![Email::try_from("user2@example.com").unwrap()])
//!     .body("This is a test email.")
//!     .build()?;
//! email.send(msg).await?;
//! # Ok(()) }
//! ```
use cot::config::EmailUrl;
use cot::email::EmailMessage;
use lettre::transport::smtp;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    /// An error occured while creating the transport.
    #[error("{ERROR_PREFIX} transport creation error: {0}")]
    TransportCreation(Box<dyn std::error::Error + Send + Sync>),
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

impl From<Mechanism> for smtp::authentication::Mechanism {
    fn from(mechanism: Mechanism) -> Self {
        match mechanism {
            Mechanism::Plain => smtp::authentication::Mechanism::Plain,
            Mechanism::Login => smtp::authentication::Mechanism::Login,
            Mechanism::Xoauth2 => smtp::authentication::Mechanism::Xoauth2,
        }
    }
}

/// SMTP transport backend that sends emails via a remote SMTP server.
///
/// # Examples
///
/// ```no_run
/// use cot::email::EmailMessage;
/// use cot::email::transport::Transport;
/// use cot::email::transport::smtp::{Smtp, Mechanism};
/// use cot::common_types::Email;
/// use cot::config::EmailUrl;
///
/// # async fn run() -> cot::Result<()> {
/// let url = EmailUrl::from("smtps://username:password@smtp.gmail.com?tls=required");
/// let smtp = Smtp::new(&url, Mechanism::Plain)?;
/// let email = cot::email::Email::new(smtp);
///
/// let msg = EmailMessage::builder()
///  .from(Email::try_from("testfrom@example.com").unwrap())
/// .to(vec![Email::try_from("testreceipient@example.com").unwrap()])
/// .body("This is a test email.")
/// .build()?;
/// email.send(msg).await?;
/// # Ok(()) }
#[derive(Debug, Clone)]
pub struct Smtp {
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl Smtp {
    /// Create a new SMTP transport backend.
    ///
    /// # Errors
    ///
    /// Returns a `TransportError` if the Smtp backend creation failed.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::EmailUrl;
    /// use cot::email::transport::smtp::{Mechanism, Smtp};
    ///
    /// let url = EmailUrl::from("smtps://username:password@smtp.gmail.com?tls=required");
    /// let smtp = Smtp::new(&url, Mechanism::Plain);
    /// ```
    pub fn new(url: &EmailUrl, mechanism: Mechanism) -> TransportResult<Self> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url.as_str())
            .map_err(|err| SMTPError::TransportCreation(Box::new(err)))?
            .authentication(vec![mechanism.into()])
            .build();

        Ok(Smtp { transport })
    }
}

impl Transport for Smtp {
    async fn send(&self, messages: &[EmailMessage]) -> TransportResult<()> {
        for message in messages {
            let m = Message::try_from(message.clone())?;
            self.transport
                .send(m)
                .await
                .map_err(|err| SMTPError::SmtpSend(Box::new(err)))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use lettre::transport::smtp;

    use super::*; // ensure access to lettre's Mechanism in this scope

    #[cot::test]
    async fn test_smtp_creation() {
        let url = EmailUrl::from("smtp://user:pass@smtp.gmail.com:587");
        let smtp = Smtp::new(&url, Mechanism::Plain);
        assert!(smtp.is_ok());
    }

    #[cot::test]
    async fn test_smtp_error_to_transport_error() {
        let smtp_error = SMTPError::SmtpSend(Box::new(std::io::Error::other("test")));
        let transport_error: TransportError = smtp_error.into();
        assert_eq!(
            transport_error.to_string(),
            "email transport error: transport error: smtp transport error: send error: test"
        );

        let smtp_error = SMTPError::TransportCreation(Box::new(std::io::Error::other("test")));
        let transport_error: TransportError = smtp_error.into();
        assert_eq!(
            transport_error.to_string(),
            "email transport error: transport error: smtp transport error: transport creation error: test"
        );

        let smtp_error = SMTPError::Io(std::io::Error::other("test"));
        let transport_error: TransportError = smtp_error.into();
        assert_eq!(
            transport_error.to_string(),
            "email transport error: transport error: smtp transport error: IO error: test"
        );
    }

    #[cot::test]
    async fn mechanism_from_maps_all_cases() {
        let m_plain: smtp::authentication::Mechanism = Mechanism::Plain.into();
        assert_eq!(m_plain, smtp::authentication::Mechanism::Plain);

        let m_login: smtp::authentication::Mechanism = Mechanism::Login.into();
        assert_eq!(m_login, smtp::authentication::Mechanism::Login);

        let m_xoauth2: smtp::authentication::Mechanism = Mechanism::Xoauth2.into();
        assert_eq!(m_xoauth2, smtp::authentication::Mechanism::Xoauth2);
    }
}
