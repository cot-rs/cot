use std::io::Write;
use std::{fmt, io};

use cot::email::EmailMessage;
use cot::email::transport::TransportError;
use thiserror::Error;

use crate::email::transport::{Transport, TransportResult};

const ERROR_PREFIX: &str = "console transport error:";

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConsoleError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

impl From<ConsoleError> for TransportError {
    fn from(err: ConsoleError) -> Self {
        TransportError::Transport(err.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct Console;

impl Console {
    pub fn new() -> Self {
        Self {}
    }
}

impl Transport for Console {
    async fn send(&self, messages: &[EmailMessage]) -> TransportResult<()> {
        let mut out = io::stdout().lock();
        for (i, msg) in messages.iter().enumerate() {
            writeln!(out, "{}", msg).map_err(|err| ConsoleError::Io(err))?;
            writeln!(out, "{}", "─".repeat(60)).map_err(|err| ConsoleError::Io(err))?;
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
                    .map(|a| format!("{}", a.email()))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };

        writeln!(
            f,
            "{}",
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
            "{}",
            "────────────────────────────────────────────────────────"
        )?;
        if self.body.trim().is_empty() {
            writeln!(f, "<empty>")?;
        } else {
            writeln!(f, "{}", self.body.trim_end())?;
        }
        writeln!(
            f,
            "{}",
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
            "{}",
            "════════════════════════════════════════════════════════════════"
        )?;
        Ok(())
    }
}
