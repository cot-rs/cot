//! Email sending functionality for Cot.
//!
//! This module exposes a high-level `Email` API that can send
//! [`EmailMessage`] values through a chosen transport backend
//! (see [`transport`] submodule for available backends).
//!
//! # Examples
//!
//! Send using the console transport backend (prints nicely formatted messages):
//!
//! ```
//! use cot::common_types::Email;
//! use cot::email::EmailMessage;
//! use cot::email::transport::console::Console;
//!
//! # async fn run() -> cot::Result<()> {
//! let email = cot::email::Email::new(Console::new());
//! let message = EmailMessage::builder()
//!     .from(Email::try_from("no-reply@example.com").unwrap())
//!     .to(vec![Email::try_from("user@example.com").unwrap()])
//!     .subject("Greetings")
//!     .body("Hello from cot!")
//!     .build()?;
//! email.send(message).await?;
//! # Ok(()) }
//! ```

pub mod transport;

use std::error::Error as StdError;
use std::sync::Arc;

use cot::config::{EmailConfig, EmailTransportTypeConfig};
use cot::email::transport::smtp::Smtp;
use cot_core::error::impl_into_cot_error;
use derive_builder::Builder;
use derive_more::with_trait::Debug;
use thiserror::Error;
use transport::{BoxedTransport, Transport};

use crate::email::transport::TransportError;
use crate::email::transport::console::Console;
const ERROR_PREFIX: &str = "email message build error:";

/// Represents errors that can occur when sending an email.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EmailError {
    /// An error occurred in the transport layer while sending the email.
    #[error(transparent)]
    Transport(TransportError),
}

impl_into_cot_error!(EmailError);

/// A convenience alias for results returned by email operations.
pub type EmailResult<T> = Result<T, EmailError>;

/// Raw attachment data to be embedded into an email.
#[derive(Debug, Clone)]
pub struct AttachmentData {
    /// The filename to display for the attachment.
    pub filename: String,
    /// The MIME content type of the attachment (e.g., `image/png`).
    pub content_type: String,
    /// The raw bytes of the attachment.
    pub data: Vec<u8>,
}

/// A high-level email message representation.
///
/// This struct encapsulates the components of an email, including
/// subject, body, sender, recipients, and attachments.
#[derive(Debug, Clone, Builder)]
#[builder(build_fn(skip))]
pub struct EmailMessage {
    /// The subject of the email.
    #[builder(setter(into))]
    subject: String,
    /// The body content of the email.
    #[builder(setter(into))]
    body: String,
    /// The sender's email address.
    from: crate::common_types::Email,
    /// The primary recipients of the email.
    to: Vec<crate::common_types::Email>,
    /// The carbon copy (CC) recipients of the email.
    cc: Vec<crate::common_types::Email>,
    /// The blind carbon copy (BCC) recipients of the email.
    bcc: Vec<crate::common_types::Email>,
    /// The reply-to addresses for the email.
    reply_to: Vec<crate::common_types::Email>,
    /// Attachments to include with the email.
    attachments: Vec<AttachmentData>,
}

impl EmailMessage {
    /// Create a new builder for constructing an [`EmailMessage`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Email;
    /// use cot::email::EmailMessage;
    ///
    /// let message = EmailMessage::builder()
    ///     .from(Email::try_from("no-reply@example.com").unwrap())
    ///     .to(vec![Email::try_from("user@example.com").unwrap()])
    ///     .subject("Greetings")
    ///     .body("Hello from cot!")
    ///     .build()
    ///     .unwrap();
    /// ```
    #[must_use]
    pub fn builder() -> EmailMessageBuilder {
        EmailMessageBuilder::default()
    }
}

impl EmailMessageBuilder {
    /// Build the [`EmailMessage`], ensuring required fields are set.
    ///
    /// # Errors
    ///
    /// This method returns an [`EmailMessageError`] if required fields are
    /// missing.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Email;
    /// use cot::email::EmailMessage;
    ///
    /// let message = EmailMessage::builder()
    ///     .from(Email::try_from("no-reply@example.com").unwrap())
    ///     .to(vec![Email::try_from("user@example.com").unwrap()])
    ///     .subject("Greetings")
    ///     .body("Hello from cot!")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn build(&self) -> Result<EmailMessage, EmailMessageError> {
        let from = self
            .from
            .clone()
            .ok_or_else(|| EmailMessageError::MissingField("from".to_string()))?;

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

/// Errors that can occur while building an email message.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EmailMessageError {
    /// An invalid email address was provided.
    #[error("{ERROR_PREFIX} invalid email address: {0}")]
    InvalidEmailAddress(Box<dyn StdError + Send + Sync + 'static>),
    /// Failed to build the email message.
    #[error("{ERROR_PREFIX} failed to build email message: {0}")]
    BuildError(Box<dyn StdError + Send + Sync + 'static>),
    /// A required field is missing in the email message.
    #[error("{ERROR_PREFIX} The `{0}` field is required but was not set")]
    MissingField(String),
}

impl_into_cot_error!(EmailMessageError);

#[derive(Debug)]
struct EmailImpl {
    #[debug("..")]
    transport: Box<dyn BoxedTransport>,
}

/// A high-level email interface for sending emails.
///
/// This struct wraps a [`Transport`] implementation and provides
///  convenient methods for sending single or multiple email messages.
///
/// # Examples
///
/// ```
/// use cot::common_types::Email;
/// use cot::email::EmailMessage;
/// use cot::email::transport::console::Console;
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let email = cot::email::Email::new(Console::new());
/// let message = EmailMessage::builder()
///     .from(Email::try_from("no-reply@example.com").unwrap())
///     .to(vec![Email::try_from("user@example.com").unwrap()])
///     .subject("Greetings")
///     .body("Hello from cot!")
///     .build()?;
/// email.send(message).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Email {
    inner: Arc<EmailImpl>,
}

impl Email {
    /// Create a new email sender using the given transport implementation.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::email::transport::console::Console;
    /// use cot::email::{Email, EmailMessage};
    ///
    /// let email = Email::new(Console::new());
    /// ```
    pub fn new(transport: impl Transport) -> Self {
        let transport: Box<dyn BoxedTransport> = Box::new(transport);
        Self {
            inner: Arc::new(EmailImpl { transport }),
        }
    }
    /// Send a single [`EmailMessage`]
    ///
    /// # Errors
    ///
    /// Returns an [`EmailError::Transport`] error if sending the email fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Email;
    /// use cot::email::EmailMessage;
    /// use cot::email::transport::console::Console;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let email = cot::email::Email::new(Console::new());
    /// let message = EmailMessage::builder()
    ///     .from(Email::try_from("no-reply@example.com").unwrap())
    ///     .to(vec![Email::try_from("user@example.com").unwrap()])
    ///     .subject("Greetings")
    ///     .body("Hello from cot!")
    ///     .build()?;
    /// email.send(message).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send(&self, message: EmailMessage) -> EmailResult<()> {
        self.inner
            .transport
            .send(&[message])
            .await
            .map_err(EmailError::Transport)
    }

    /// Send multiple emails in sequence.
    ///
    /// # Errors
    ///
    /// Returns an [`EmailError::Transport`] if sending any of the emails fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Email;
    /// use cot::email::EmailMessage;
    /// use cot::email::transport::console::Console;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let email = cot::email::Email::new(Console::new());
    /// let message1 = EmailMessage::builder()
    ///     .from(Email::try_from("no-reply@email.com").unwrap())
    ///     .to(vec![Email::try_from("user1@example.com").unwrap()])
    ///     .subject("Hello User 1")
    ///     .body("This is the first email.")
    ///     .build()?;
    ///
    /// let message2 = EmailMessage::builder()
    ///     .from(Email::try_from("no-reply@email.com").unwrap())
    ///     .to(vec![Email::try_from("user2@example.com").unwrap()])
    ///     .subject("Hello User 2")
    ///     .body("This is the second email.")
    ///     .build()?;
    /// email.send_multiple(&[message1, message2]).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_multiple(&self, messages: &[EmailMessage]) -> EmailResult<()> {
        self.inner
            .transport
            .send(messages)
            .await
            .map_err(EmailError::Transport)
    }

    /// Construct an [`Email`] from the provided [`EmailConfig`].
    ///
    /// # Errors
    ///
    /// Returns an [`EmailError::Transport`] error if creating the transport
    /// backend fails from the config.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::{EmailConfig, EmailTransportConfig, EmailTransportTypeConfig};
    /// use cot::email::Email;
    /// use cot::email::transport::console::Console;
    ///
    /// let config = EmailConfig {
    ///     transport: EmailTransportConfig::builder()
    ///         .transport_type(EmailTransportTypeConfig::Console)
    ///         .build(),
    ///     ..Default::default()
    /// };
    /// let email = Email::from_config(&config);
    /// ```
    pub fn from_config(config: &EmailConfig) -> EmailResult<Self> {
        let transport = &config.transport;

        let this = {
            match &transport.transport_type {
                EmailTransportTypeConfig::Console => {
                    let console = Console::new();
                    Self::new(console)
                }

                EmailTransportTypeConfig::Smtp { url, mechanism } => {
                    let smtp = Smtp::new(url, *mechanism).map_err(EmailError::Transport)?;
                    Self::new(smtp)
                }
            }
        };
        Ok(this)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{EmailTransportConfig, EmailUrl};
    use crate::email::transport::smtp::Mechanism;

    #[cot::test]
    async fn builder_errors_when_from_missing() {
        let res = EmailMessage::builder()
            .subject("Hello".to_string())
            .body("World".to_string())
            .build();
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert_eq!(
            err.to_string(),
            "email message build error: The `from` field is required but was not set"
        );
    }

    #[cot::test]
    async fn builder_defaults_when_only_from_set() {
        let msg = EmailMessage::builder()
            .from(crate::common_types::Email::new("sender@example.com").unwrap())
            .build()
            .expect("should build with defaults");
        assert_eq!(msg.subject, "");
        assert_eq!(msg.body, "");
        assert!(msg.to.is_empty());
        assert!(msg.cc.is_empty());
        assert!(msg.bcc.is_empty());
        assert!(msg.reply_to.is_empty());
        assert!(msg.attachments.is_empty());
    }

    #[cot::test]
    async fn from_config_console_builds() {
        use crate::config::{EmailConfig, EmailTransportTypeConfig};
        let cfg = EmailConfig {
            transport: EmailTransportConfig {
                transport_type: EmailTransportTypeConfig::Console,
            },
        };
        let email = Email::from_config(&cfg);
        assert!(email.is_ok());
    }

    #[cot::test]
    async fn from_config_smtp_builds() {
        let cfg = EmailConfig {
            transport: EmailTransportConfig {
                transport_type: EmailTransportTypeConfig::Smtp {
                    url: EmailUrl::from("smtp://localhost:1025"),
                    mechanism: Mechanism::Plain,
                },
            },
        };
        let email = Email::from_config(&cfg);
        assert!(email.is_ok());
    }

    #[cot::test]
    async fn email_send_console() {
        let console = Console::new();
        let email = Email::new(console);
        let msg = EmailMessage::builder()
            .from(crate::common_types::Email::new("user@example.com").unwrap())
            .to(vec![
                crate::common_types::Email::new("recipient@example.com").unwrap(),
            ])
            .subject("Test Email".to_string())
            .body("This is a test email body.".to_string())
            .build()
            .unwrap();

        assert!(email.send(msg).await.is_ok());
    }

    #[cot::test]
    async fn email_send_multiple_console() {
        let console = Console::new();
        let email = Email::new(console);
        let msg1 = EmailMessage::builder()
            .from(crate::common_types::Email::new("user1@example.com").unwrap())
            .to(vec![
                crate::common_types::Email::new("recipient@example.com").unwrap(),
            ])
            .subject("Test Email")
            .body("This is a test email body.")
            .build()
            .unwrap();

        let msg2 = EmailMessage::builder()
            .from(crate::common_types::Email::new("user2@example.com").unwrap())
            .to(vec![
                crate::common_types::Email::new("user2@example.com").unwrap(),
            ])
            .subject("Another Test Email")
            .body("This is another test email body.")
            .build()
            .unwrap();
        assert!(email.send_multiple(&[msg1, msg2]).await.is_ok());
    }
}
