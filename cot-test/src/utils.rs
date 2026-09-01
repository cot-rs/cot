//! Test utilities

use std::fmt::Write;

/// Formats a code snippet for an error message using the style of rustc
/// diagnostics
///
/// # Example
///
/// ```text
///    --> migrations.md:201:1
///     |
/// 201 | Rollback dry run
/// 202 |
/// 203 |    Target:
/// 204 |      app:       customers
/// 205 |      migration: m_0001_initial
///     |
/// ```
#[must_use]
pub fn format_code_snippet(
    literal: &str,
    file_name: &str,
    start_line: usize,
    start_col: usize,
    max_lines: usize,
) -> String {
    const FMT_MSG: &str = "failed to write to string buffer";
    let lines: Vec<&str> = literal.lines().take(max_lines).collect();

    // Width of the largest line number, for gutter alignment.
    let last_line_num = start_line + lines.len().saturating_sub(1);
    let gutter_width = last_line_num.to_string().len();

    let mut out = String::new();
    writeln!(
        out,
        "{:width$}--> {}:{}:{}\n",
        "",
        file_name,
        start_line,
        start_col,
        width = gutter_width + 1
    )
    .expect(FMT_MSG);
    writeln!(out, "{:gutter_width$} |", "").expect(FMT_MSG);

    for (i, line) in lines.iter().enumerate() {
        let line_num = start_line + i;
        writeln!(out, "{line_num:gutter_width$} | {line}").expect(FMT_MSG);
    }
    writeln!(out, "{:gutter_width$} |", "").expect(FMT_MSG);

    out
}
