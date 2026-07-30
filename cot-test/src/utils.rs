//! Test utilities

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
pub fn format_code_snippet(
    literal: &str,
    file_name: &str,
    start_line: usize,
    start_col: usize,
    max_lines: usize,
) -> String {
    let lines: Vec<&str> = literal.lines().take(max_lines).collect();

    // Width of the largest line number, for gutter alignment.
    let last_line_num = start_line + lines.len().saturating_sub(1);
    let gutter_width = last_line_num.to_string().len();

    let mut out = String::new();
    out.push_str(&format!(
        "{:width$}--> {}:{}:{}\n",
        "",
        file_name,
        start_line,
        start_col,
        width = gutter_width + 1
    ));
    out.push_str(&format!("{:width$} |\n", "", width = gutter_width));

    for (i, line) in lines.iter().enumerate() {
        let line_num = start_line + i;
        out.push_str(&format!(
            "{line_num:width$} | {line}\n",
            width = gutter_width
        ));
    }

    out.push_str(&format!("{:width$} |", "", width = gutter_width));

    out
}
