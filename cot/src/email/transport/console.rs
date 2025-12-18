//! Console transport implementation.
//!
//! This backend writes a human-friendly representation of emails to stdout.
//! It is intended primarily for development and testing environments where
//! actually sending email is not required.
//!
//! Typical usage is through the high-level [`crate::email::Email`] API:
//!
//! ```no_run
//! use cot::common_types::Email;
//! use cot::email::EmailMessage;
//! use cot::email::transport::console::Console;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let email = cot::email::Email::new(Console::new());
//! let recipients = vec![Email::try_from("testreceipient@example.com").unwrap()];
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
        TransportError::Backend(err.to_string())
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

    #[cot::test]
    async fn console_error_to_transport_error() {
        let console_error = ConsoleError::Io(io::Error::other("test error"));
        let transport_error: TransportError = console_error.into();

        assert_eq!(
            transport_error.to_string(),
            "email transport error: transport error: console transport error: IO error: test error"
        );
    }
}
