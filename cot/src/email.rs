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
use derive_builder::Builder;
use derive_more::with_trait::Debug;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Body, Mailbox, Message, MultiPart, SinglePart};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use transport::{BoxedTransport, Transport};

use crate::common_types::Password;
use crate::email::transport::console::Console;
use crate::email::transport::smtp::SMTPCredentials;
use crate::error::error_impl::impl_into_cot_error;

const ERROR_PREFIX: &str = "email error:";

/// Represents errors that can occur when sending an email.
#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("transport error: {0}")]
    TransportError(String),
    /// An error occurred while building the email message.
    #[error("message error: {0}")]
    MessageError(String),
    /// A required field is missing in the email message.
    #[error("missing required field: {0}")]
    MissingField(String),
}

impl_into_cot_error!(EmailError);
pub type EmailResult<T> = Result<T, EmailError>;

#[derive(Debug, Clone)]
pub struct AttachmentData {
    filename: String,
    content_type: String,
    data: Vec<u8>,
}

#[derive(Debug, Clone, Builder)]
#[builder(build_fn(skip))]
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

impl EmailMessage {
    #[must_use]
    pub fn builder() -> EmailMessageBuilder {
        EmailMessageBuilder::default()
    }
}

impl EmailMessageBuilder {
    pub fn build(&self) -> Result<EmailMessage, EmailError> {
        let from = self
            .from
            .clone()
            .ok_or_else(|| EmailError::MissingField("from".to_string()))?;

        let subject = self.subject.clone().unwrap_or_default();
        let body = self.body.clone().unwrap_or_default();

        let to = self.to.clone().unwrap_or_default();
        let cc = self.cc.clone().unwrap_or_default();
        let bcc = self.bcc.clone().unwrap_or_default();
        let reply_to = self.reply_to.clone().unwrap_or_default();
        let attachments = self.attachments.clone().unwrap_or_default();

        Ok(EmailMessage {
            subject,
            body,
            from,
            to,
            cc,
            bcc,
            reply_to,
            attachments,
        })
    }
}

#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum MessageBuildError {
    #[error("invalid email address: {0}")]
    InvalidEmailAddress(String),
    #[error("failed to build email message: {0}")]
    BuildError(String),
}

impl TryFrom<EmailMessage> for Message {
    type Error = MessageBuildError;

    fn try_from(message: EmailMessage) -> Result<Self, Self::Error> {
        let from_mailbox = message
            .from
            .email()
            .as_str()
            .parse::<Mailbox>()
            .map_err(|err| MessageBuildError::InvalidEmailAddress(err.to_string()))?;

        let mut builder = Message::builder()
            .from(from_mailbox)
            .subject(message.subject);

        for to in message.to {
            let mb = to
                .email()
                .as_str()
                .parse::<Mailbox>()
                .map_err(|err| MessageBuildError::InvalidEmailAddress(err.to_string()))?;
            builder = builder.to(mb);
        }

        for cc in message.cc {
            let mb = cc
                .email()
                .as_str()
                .parse::<Mailbox>()
                .map_err(|err| MessageBuildError::InvalidEmailAddress(err.to_string()))?;
            builder = builder.cc(mb);
        }

        for bcc in message.bcc {
            let mb = bcc
                .email()
                .as_str()
                .parse::<Mailbox>()
                .map_err(|err| MessageBuildError::InvalidEmailAddress(err.to_string()))?;
            builder = builder.bcc(mb);
        }

        for r in message.reply_to {
            let mb = r
                .email()
                .as_str()
                .parse::<Mailbox>()
                .map_err(|err| MessageBuildError::InvalidEmailAddress(err.to_string()))?;
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
            .map_err(|err| MessageBuildError::BuildError(err.to_string()))?;
        Ok(email)
    }
}

#[derive(Debug, Clone)]
pub struct Email {
    #[debug("..")]
    transport: Arc<dyn BoxedTransport>,
}

impl Email {
    pub fn new(transport: impl Transport) -> Self {
        let transport: Arc<dyn BoxedTransport> = Arc::new(transport);
        Self { transport }
    }
    pub async fn send(&self, messages: &[EmailMessage]) -> EmailResult<()> {
        self.transport
            .send(messages)
            .await
            .map_err(|err| EmailError::TransportError(err.to_string()))
    }

    pub fn from_config(config: &EmailConfig) -> Self {
        let transport = &config.transport;

        let this = {
            match &transport.transport_type {
                EmailTransportTypeConfig::Console => {
                    let console = Console::new();
                    Self::new(console)
                }

                EmailTransportTypeConfig::Smtp {
                    auth_id,
                    secret,
                    mechanism,
                    server: host,
                } => {
                    let credentials =
                        SMTPCredentials::new(auth_id.clone(), Password::from(secret.clone()));
                    let smtp = SMTP::new(credentials, host.clone(), mechanism.clone());
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
    }
}
