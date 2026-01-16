//! SMTP transport implementation.
//!
//! This backend sends email messages to a configured remote SMTP
//! server.
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
use std::error::Error as StdError;

use cot::config::EmailUrl;
use cot::email::{EmailMessage, EmailMessageError};
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Body, Mailbox, MultiPart, SinglePart};
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
    SmtpSend(Box<dyn StdError + Send + Sync>),
    /// An error occurred while creating the transport.
    #[error("{ERROR_PREFIX} transport creation error: {0}")]
    TransportCreation(Box<dyn StdError + Send + Sync>),
    /// An error occurred while building the email message.
    #[error("{ERROR_PREFIX} message error: {0}")]
    MessageBuild(#[from] EmailMessageError),
}

impl From<SMTPError> for TransportError {
    fn from(err: SMTPError) -> Self {
        match err {
            SMTPError::MessageBuild(e) => TransportError::MessageBuildError(e),
            other => TransportError::Backend(Box::new(other)),
        }
    }
}

/// Supported SMTP authentication mechanisms.
///
/// The default is `Plain`.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Mechanism {
    /// PLAIN authentication mechanism defined in [RFC 4616](https://tools.ietf.org/html/rfc4616)
    /// This is the default authentication mechanism.
    #[default]
    Plain,
    /// LOGIN authentication mechanism defined in
    /// [draft-murchison-sasl-login-00](https://www.ietf.org/archive/id/draft-murchison-sasl-login-00.txt).
    /// This mechanism is obsolete but needed for some providers (like Office
    /// 365).
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
/// # #[tokio::main]
/// # async fn run() -> cot::Result<()> {
/// let url = EmailUrl::from("smtps://johndoe:xxxx xxxxx xxxx xxxxx@smtp.gmail.com");
/// let smtp = Smtp::new(&url, Mechanism::Plain)?;
/// let email = cot::email::Email::new(smtp);
///
/// let msg = EmailMessage::builder()
///     .from(Email::try_from("testfrom@example.com").unwrap())
///     .to(vec![Email::try_from("testreceipient@example.com").unwrap()])
///     .body("This is a test email.")
///     .build()?;
/// email.send(msg).await?;
/// # Ok(()) }
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Smtp {
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl Smtp {
    /// Create a new SMTP transport backend.
    ///
    /// # Errors
    ///
    /// Returns an [`SMTPError::TransportCreation`] if the SMTP backend creation
    /// failed.
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
            let m = convert_email_message_to_lettre_message(message.clone())?;
            self.transport
                .send(m)
                .await
                .map_err(|err| SMTPError::SmtpSend(Box::new(err)))?;
        }
        Ok(())
    }
}

fn convert_email_message_to_lettre_message(
    message: EmailMessage,
) -> Result<Message, EmailMessageError> {
    let from_mailbox = message
        .from
        .email()
        .as_str()
        .parse::<Mailbox>()
        .map_err(|err| EmailMessageError::InvalidEmailAddress(Box::new(err)))?;

    let mut builder = Message::builder()
        .from(from_mailbox)
        .subject(message.subject);

    for to in message.to {
        let mb = to
            .email()
            .as_str()
            .parse::<Mailbox>()
            .map_err(|err| EmailMessageError::InvalidEmailAddress(Box::new(err)))?;
        builder = builder.to(mb);
    }

    for cc in message.cc {
        let mb = cc
            .email()
            .as_str()
            .parse::<Mailbox>()
            .map_err(|err| EmailMessageError::InvalidEmailAddress(Box::new(err)))?;
        builder = builder.cc(mb);
    }

    for bcc in message.bcc {
        let mb = bcc
            .email()
            .as_str()
            .parse::<Mailbox>()
            .map_err(|err| EmailMessageError::InvalidEmailAddress(Box::new(err)))?;
        builder = builder.bcc(mb);
    }

    for r in message.reply_to {
        let mb = r
            .email()
            .as_str()
            .parse::<Mailbox>()
            .map_err(|err| EmailMessageError::InvalidEmailAddress(Box::new(err)))?;
        builder = builder.reply_to(mb);
    }

    let mut mixed = MultiPart::mixed().singlepart(SinglePart::plain(message.body));

    for attach in message.attachments {
        let mime: ContentType = attach.content_type.parse().unwrap_or_else(|_| {
            "application/octet-stream"
                .parse()
                .expect("could not parse default mime type")
        });

        let part = Attachment::new(attach.filename).body(Body::new(attach.data), mime);
        mixed = mixed.singlepart(part);
    }

    let email = builder
        .multipart(mixed)
        .map_err(|err| EmailMessageError::BuildError(Box::new(err)))?;
    Ok(email)
}

#[cfg(test)]
mod tests {
    use cot::email::AttachmentData;
    use lettre::transport::smtp;

    use super::*;

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

    #[cot::test]
    async fn try_from_basic_converts_and_contains_headers() {
        let msg = EmailMessage::builder()
            .from(crate::common_types::Email::new("from@example.com").unwrap())
            .to(vec![
                crate::common_types::Email::new("to@example.com").unwrap(),
            ])
            .subject("Hello World")
            .body("This is the body.")
            .build()
            .unwrap();

        let built: Message =
            convert_email_message_to_lettre_message(msg).expect("conversion to lettre::Message");

        let formatted = String::from_utf8_lossy(&built.formatted()).to_string();

        assert!(formatted.contains("From: from@example.com"),);
        assert!(formatted.contains("To: to@example.com"),);
        assert!(formatted.contains("Subject: Hello World"),);
        assert!(formatted.contains("Content-Type: multipart/mixed"),);
        assert!(formatted.contains("This is the body."),);
    }

    #[cot::test]
    async fn try_from_includes_cc_and_reply_to_headers() {
        let msg = EmailMessage::builder()
            .from(crate::common_types::Email::new("sender@example.com").unwrap())
            .to(vec![
                crate::common_types::Email::new("primary@example.com").unwrap(),
            ])
            .cc(vec![
                crate::common_types::Email::new("cc1@example.com").unwrap(),
                crate::common_types::Email::new("cc2@example.com").unwrap(),
            ])
            .bcc(vec![
                crate::common_types::Email::new("hidden@example.com").unwrap(),
            ])
            .reply_to(vec![
                crate::common_types::Email::new("replyto@example.com").unwrap(),
            ])
            .subject("Headers Test")
            .body("Body")
            .build()
            .unwrap();

        let built: Message =
            convert_email_message_to_lettre_message(msg).expect("conversion to lettre::Message");
        let formatted = String::from_utf8_lossy(&built.formatted()).to_string();

        assert!(
            formatted.contains("Cc: cc1@example.com, cc2@example.com")
                || (formatted.contains("Cc: cc1@example.com")
                    && formatted.contains("cc2@example.com")),
        );
        assert!(formatted.contains("Reply-To: replyto@example.com"),);
    }

    #[cot::test]
    async fn try_from_with_attachment_uses_default_mime_on_parse_failure() {
        let attachment = AttachmentData {
            filename: "report.bin".to_string(),
            content_type: "this/is not a valid mime".to_string(),
            data: vec![0xDE, 0xAD, 0xBE, 0xEF],
        };

        let msg = EmailMessage::builder()
            .from(crate::common_types::Email::new("sender@example.com").unwrap())
            .to(vec![
                crate::common_types::Email::new("to@example.com").unwrap(),
            ])
            .subject("Attachment Test")
            .body("Please see attachment")
            .attachments(vec![attachment])
            .build()
            .unwrap();

        let built: Message =
            convert_email_message_to_lettre_message(msg).expect("conversion to lettre::Message");
        let formatted = String::from_utf8_lossy(&built.formatted()).to_string();

        assert!(formatted.contains("Content-Disposition: attachment"),);
        assert!(formatted.contains("report.bin"),);
        assert!(formatted.contains("Content-Type: application/octet-stream"),);
        assert!(formatted.contains("Please see attachment"));
    }
}
