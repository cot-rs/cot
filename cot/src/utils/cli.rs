//! Utilities for CLI
use anstyle::{AnsiColor, Color, Effects, Style};

#[doc(hidden)] // Not part of Cot's public API; used by the CLI.
pub fn print_status_msg(status: StatusType, message: &str) {
    let style = status.style();
    let status_str = status.as_str();

    eprintln!("{style}{status_str:>12}{style:#} {message}");
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
#[non_exhaustive] // Not part of Cot's public API; used by the CLI.
pub enum StatusType {
    // In-Progress Ops
    Creating,
    Adding,
    Modifying,
    Removing,
    RollingBack,
    // Completed Ops
    Created,
    Added,
    Modified,
    Removed,
    RolledBack,

    // Status types
    Error,   // Should be used in Error handling inside remove operations
    Warning, // Should be used as cautionary messages.
    Notice,
}

impl StatusType {
    fn style(self) -> Style {
        let base_style = Style::new() | Effects::BOLD;

        match self {
            // In-Progress => Brighter colors
            StatusType::Creating => base_style.fg_color(Some(Color::Ansi(AnsiColor::BrightGreen))),
            StatusType::Adding => base_style.fg_color(Some(Color::Ansi(AnsiColor::BrightCyan))),
            StatusType::Removing | StatusType::RollingBack => {
                base_style.fg_color(Some(Color::Ansi(AnsiColor::BrightMagenta)))
            }
            StatusType::Modifying => base_style.fg_color(Some(Color::Ansi(AnsiColor::BrightBlue))),
            // Completed => Dimmed colors
            StatusType::Created => base_style.fg_color(Some(Color::Ansi(AnsiColor::Green))),
            StatusType::Added => base_style.fg_color(Some(Color::Ansi(AnsiColor::Cyan))),
            StatusType::Removed | StatusType::RolledBack => {
                base_style.fg_color(Some(Color::Ansi(AnsiColor::Magenta)))
            }
            StatusType::Modified => base_style.fg_color(Some(Color::Ansi(AnsiColor::Blue))),
            // Status types
            StatusType::Warning => base_style.fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
            StatusType::Error => base_style.fg_color(Some(Color::Ansi(AnsiColor::Red))),
            StatusType::Notice => base_style.fg_color(Some(Color::Ansi(AnsiColor::White))),
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            StatusType::Creating => "Creating",
            StatusType::Adding => "Adding",
            StatusType::Modifying => "Modifying",
            StatusType::Removing => "Removing",
            StatusType::Created => "Created",
            StatusType::Added => "Added",
            StatusType::Modified => "Modified",
            StatusType::Removed => "Removed",
            StatusType::Warning => "Warning",
            StatusType::Error => "Error",
            StatusType::Notice => "Notice",
            StatusType::RollingBack => "Rolling Back",
            StatusType::RolledBack => "Rolled Back",
        }
    }
}
