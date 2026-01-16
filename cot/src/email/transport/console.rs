//! Console transport implementation.
//!
//! This backend writes a human-friendly representation of emails to stdout.
//! It is intended primarily for development and testing environments where
//! actually sending email is not required.
//!
//! Typical usage is through the high-level [`crate::email::Email`] API.
//!
//! ## Examples
//!
//! ```
//! use cot::common_types::Email;
//! use cot::email::EmailMessage;
//! use cot::email::transport::console::Console;
//!
//! # #[tokio::main]
//! # async fn main() -> cot::Result<()> {
//! let email = cot::email::Email::new(Console::new());
//! let recipients = vec![Email::try_from("testrecipient@example.com").unwrap()];
//! let msg = EmailMessage::builder()
//!     .from(Email::try_from("no-reply@example.com").unwrap())
//!     .to(vec![Email::try_from("user@example.com").unwrap()])
//!     .build()?;
//! email.send(msg).await?;
//! # Ok(()) }
//! ```
use std::io::Write;
use std::{fmt, io};

use cot::email::EmailMessage;
use cot::email::transport::TransportError;
use thiserror::Error;

use crate::email::transport::{Transport, TransportResult};

const ERROR_PREFIX: &str = "console transport error:";

/// Errors that can occur while using the console transport.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConsoleError {
    /// An IO error occurred while writing to stdout.
    #[error("{ERROR_PREFIX} IO error: {0}")]
    Io(#[from] io::Error),
}

impl From<ConsoleError> for TransportError {
    fn from(err: ConsoleError) -> Self {
        TransportError::Backend(Box::new(err))
    }
}

/// A transport backend that prints emails to stdout.
///
/// # Examples
///
/// ```
/// use cot::email::transport::console::Console;
///
/// let console_transport = Console::new();
/// ```
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Console;

impl Console {
    /// Create a new console transport backend.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::email::transport::console::Console;
    ///
    /// let console_transport = Console::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for Console {
    async fn send(&self, messages: &[EmailMessage]) -> TransportResult<()> {
        let mut out = io::stdout().lock();
        for msg in messages {
            writeln!(out, "{msg}").map_err(ConsoleError::Io)?;
            writeln!(out, "{}", "─".repeat(60)).map_err(ConsoleError::Io)?;
        }
        Ok(())
    }
}

impl fmt::Display for EmailMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fmt_list = |list: &Vec<crate::common_types::Email>| -> String {
            if list.is_empty() {
                "-".to_string()
            } else {
                list.iter()
                    .map(|a| a.email().clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };

        writeln!(
            f,
            "════════════════════════════════════════════════════════════════"
        )?;
        writeln!(f, "From    : {}", self.from.email())?;
        writeln!(f, "To      : {}", fmt_list(&self.to))?;
        if !self.cc.is_empty() {
            writeln!(f, "Cc      : {}", fmt_list(&self.cc))?;
        }
        if !self.bcc.is_empty() {
            writeln!(f, "Bcc     : {}", fmt_list(&self.bcc))?;
        }
        if !self.reply_to.is_empty() {
            writeln!(f, "Reply-To: {}", fmt_list(&self.reply_to))?;
        }
        writeln!(
            f,
            "Subject : {}",
            if self.subject.is_empty() {
                "-"
            } else {
                &self.subject
            }
        )?;
        writeln!(
            f,
            "────────────────────────────────────────────────────────"
        )?;
        if self.body.trim().is_empty() {
            writeln!(f, "<empty>")?;
        } else {
            writeln!(f, "{}", self.body.trim_end())?;
        }
        writeln!(
            f,
            "────────────────────────────────────────────────────────"
        )?;
        if self.attachments.is_empty() {
            writeln!(f, "Attachments: -")?;
        } else {
            writeln!(f, "Attachments ({}):", self.attachments.len())?;
            for a in &self.attachments {
                writeln!(
                    f,
                    "  - {} ({} bytes, {})",
                    a.filename,
                    a.data.len(),
                    a.content_type
                )?;
            }
        }
        writeln!(
            f,
            "════════════════════════════════════════════════════════════════"
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common_types::Email as Addr;
    use crate::email::{AttachmentData, Email};

    #[cot::test]
    async fn console_error_to_transport_error() {
        let console_error = ConsoleError::Io(io::Error::other("test error"));
        let transport_error: TransportError = console_error.into();

        assert_eq!(
            transport_error.to_string(),
            "email transport error: transport error: console transport error: IO error: test error"
        );
    }

    #[cot::test]
    async fn display_full_message_renders_all_sections() {
        let msg = EmailMessage::builder()
            .from(Addr::new("from@example.com").unwrap())
            .to(vec![
                Addr::new("to1@example.com").unwrap(),
                Addr::new("to2@example.com").unwrap(),
            ])
            .cc(vec![
                Addr::new("cc1@example.com").unwrap(),
                Addr::new("cc2@example.com").unwrap(),
            ])
            .bcc(vec![Addr::new("bcc@example.com").unwrap()])
            .reply_to(vec![Addr::new("reply@example.com").unwrap()])
            .subject("Subject Line")
            .body("Hello body\n")
            .attachments(vec![
                AttachmentData {
                    filename: "a.txt".into(),
                    content_type: "text/plain".into(),
                    data: b"abc".to_vec(),
                },
                AttachmentData {
                    filename: "b.pdf".into(),
                    content_type: "application/pdf".into(),
                    data: vec![0u8; 10],
                },
            ])
            .build()
            .unwrap();

        let console = Console::new();
        let email = Email::new(console);
        email
            .send(msg.clone())
            .await
            .expect("console send should succeed");

        let rendered = format!("{msg}");

        assert!(rendered.contains("From    : from@example.com"));
        assert!(rendered.contains("To      : to1@example.com, to2@example.com"));
        assert!(rendered.contains("Subject : Subject Line"));
        assert!(rendered.contains("────────────────────────────────────────────────────────"));

        assert!(rendered.contains("Cc      : cc1@example.com, cc2@example.com"));
        assert!(rendered.contains("Bcc     : bcc@example.com"));
        assert!(rendered.contains("Reply-To: reply@example.com"));

        assert!(rendered.contains("Hello body"));

        assert!(rendered.contains("Attachments (2):"));
        assert!(rendered.contains("  - a.txt (3 bytes, text/plain)"));
        assert!(rendered.contains("  - b.pdf (10 bytes, application/pdf)"));

        assert!(
            rendered.contains("════════════════════════════════════════════════════════════════")
        );
    }

    #[cot::test]
    async fn display_minimal_message_renders_placeholders_and_omits_optional_headers() {
        let msg = EmailMessage::builder()
            .from(Addr::new("sender@example.com").unwrap())
            // whitespace-only body should render as <empty>
            .body(" \t\n ")
            .build()
            .unwrap();

        let console = Console::default();
        let email = Email::new(console);
        email
            .send(msg.clone())
            .await
            .expect("console send should succeed");

        let rendered = format!("{msg}");

        assert!(rendered.contains("From    : sender@example.com"));
        assert!(rendered.contains("To      : -"));
        assert!(rendered.contains("Subject : -"));

        assert!(!rendered.contains("Cc      :"));
        assert!(!rendered.contains("Bcc     :"));
        assert!(!rendered.contains("Reply-To:"));

        assert!(rendered.contains("<empty>"));
        assert!(rendered.contains("Attachments: -"));
    }
}
