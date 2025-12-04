//! Email sending functionality using SMTP and other backends
//!
//! #Examples
//! To send an email using the `EmailBackend`, you need to create an instance of
//! `SmtpConfig`
//! ```
//! use cot::email::{EmailBackend, EmailMessage, SmtpConfig, SmtpEmailBackend};
//! fn test_send_email_localhsot() {
//!     // Create a test email
//!     let email = EmailMessage {
//!         subject: "Test Email".to_string(),
//!         from: String::from("<from@cotexample.com>").into(),
//!         to: vec!["<to@cotexample.com>".to_string()],
//!         body: "This is a test email sent from Rust.".to_string(),
//!         alternative_html: Some(
//!             "<p>This is a test email sent from Rust as HTML.</p>".to_string(),
//!         ),
//!         ..Default::default()
//!     };
//!     let config = SmtpConfig::default();
//!     // Create a new email backend
//!     let mut backend = SmtpEmailBackend::new(config);
//!     let _ = backend.send_message(&email);
//! }
//! ```

pub mod transport;

use std::error::Error;
use std::sync::Arc;

use cot::config::{EmailConfig, EmailTransportTypeConfig};
use cot::email::transport::smtp::SMTP;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Body, Mailbox, Message, MultiPart, SinglePart};
use serde::{Deserialize, Serialize};
use transport::{BoxedTransport, Transport};

use crate::config::EmailTransportTypeConfig::Smtp;
use crate::email::transport::console::Console;

/// Represents errors that can occur when sending an email.
#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    /// An error occurred while building the email message.
    #[error("Message error: {0}")]
    MessageError(String),
    /// The email configuration is invalid.
    #[error("Invalid email configuration: {0}")]
    ConfigurationError(String),
    /// An error occurred while sending the email.
    #[error("Send error: {0}")]
    SendError(String),
    /// An error occurred while connecting to the SMTP server.
    #[error("Connection error: {0}")]
    ConnectionError(String),
}

type Result<T> = std::result::Result<T, EmailError>;

pub type EmailResult<T> = std::result::Result<T, EmailError>;

#[derive(Debug, Clone)]
pub struct AttachmentData {
    filename: String,
    content_type: String,
    data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct EmailMessage {
    subject: String,
    body: String,
    from: crate::common_types::Email,
    to: Vec<crate::common_types::Email>,
    cc: Vec<crate::common_types::Email>,
    bcc: Vec<crate::common_types::Email>,
    reply_to: Vec<crate::common_types::Email>,
    attachments: Vec<AttachmentData>,
}

impl TryFrom<EmailMessage> for Message {
    type Error = EmailError;

    fn try_from(message: EmailMessage) -> Result<Self> {
        let from_mailbox: Mailbox = message.from.email().parse()?;

        let mut builder = Message::builder()
            .from(from_mailbox)
            .subject(message.subject);

        for to in message.to {
            let mb: Mailbox = to.email().parse()?;
            builder = builder.to(mb);
        }

        for cc in message.cc {
            let mb: Mailbox = cc.email().parse()?;
            builder = builder.cc(mb);
        }

        for bcc in message.bcc {
            let mb: Mailbox = bcc.email().parse()?;
            builder = builder.bcc(mb);
        }

        for r in message.reply_to {
            let mb: Mailbox = r.email().parse()?;
            builder = builder.reply_to(mb);
        }

        let mut mixed = MultiPart::mixed().singlepart(SinglePart::plain(message.body));

        for attach in message.attachments {
            let mime: ContentType = attach
                .content_type
                .parse()
                .unwrap_or_else(|_| "application/octet-stream".parse().unwrap());

            let part = Attachment::new(attach.filename).body(Body::new(attach.data), mime);
            mixed = mixed.singlepart(part);
        }

        let email = builder.multipart(mixed).map_err(|err| {
            EmailError::MessageError(format!("Failed to build email message,error:{err}"))
        })?;
        Ok(email)
    }
}

#[derive(Debug, Clone)]
pub struct Email {
    transport: Arc<dyn BoxedTransport>,
}

impl Email {
    pub fn new(transport: impl Transport) -> Self {
        let transport: Arc<dyn BoxedTransport> = Arc::new(transport);
        Self { transport }
    }
    pub fn send(&self, messages: &[EmailMessage]) -> EmailResult<()> {
        self.transport.send(messages)?
    }

    pub fn from_config(config: &EmailConfig) -> Self {
        let transport = &config.transport;

        let this = {
            match transport.transport_type {
                EmailTransportTypeConfig::Console => {
                    let console = Console::new();
                    Self::new(console)
                }

                EmailTransportTypeConfig::Smtp {
                    ref credentials,
                    host,
                } => {
                    let smtp = SMTP::new(credentials.clone(), host.clone());
                    Self::new(smtp)
                }
            }
        };
        this
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_error_variants() {
        let message_error = EmailError::MessageError("Invalid message".to_string());
        assert_eq!(format!("{message_error}"), "Message error: Invalid message");

        let config_error = EmailError::ConfigurationError("Invalid config".to_string());
        assert_eq!(
            format!("{config_error}"),
            "Invalid email configuration: Invalid config"
        );

        let send_error = EmailError::SendError("Failed to send".to_string());
        assert_eq!(format!("{send_error}"), "Send error: Failed to send");

        let connection_error = EmailError::ConnectionError("Failed to connect".to_string());
        assert_eq!(
            format!("{connection_error}"),
            "Connection error: Failed to connect"
        );
    }
}
