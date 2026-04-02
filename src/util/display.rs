//! Terminal display utilities: colored output and table formatting.

use colored::Colorize;

/// Build a formatted table from headers and rows.
pub fn format_table(headers: &[&str], rows: &[Vec<String>], column_spacing: usize) -> String {
    if headers.is_empty() {
        return String::new();
    }

    // Calculate column widths from headers
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();

    // Expand widths based on row data
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    let mut output = String::new();
    let spacing = " ".repeat(column_spacing);

    // Header row
    let header_line: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:<width$}", h, width = widths[i]))
        .collect();
    output.push_str(&header_line.join(&spacing));
    output.push('\n');

    // Separator line
    let separator_line: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    output.push_str(&separator_line.join(&spacing));
    output.push('\n');

    // Data rows
    for row in rows {
        let row_line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| format!("{:<width$}", cell, width = widths.get(i).unwrap_or(&0)))
            .collect();
        output.push_str(&row_line.join(&spacing));
        output.push('\n');
    }

    output
}

/// Print a table with colored headers.
pub fn print_table(headers: &[&str], rows: &[Vec<String>], column_spacing: usize) {
    let table = format_table(headers, rows, column_spacing);
    println!("{}", table);
}

/// Print a table with a title.
pub fn print_table_with_title(
    title: &str,
    headers: &[&str],
    rows: &[Vec<String>],
    column_spacing: usize,
) {
    println!("{}", title.green().bold());
    println!();
    print_table(headers, rows, column_spacing);
}

/// Print colored output with different log levels (Heroku/Piku style).
///
/// This follows the Heroku buildpack output convention:
/// - Info messages: "-----> " prefix, green, stdout
/// - Warnings: " !     " prefix, yellow, stderr
/// - Errors: " !     " prefix, red, stderr
pub fn echo(msg: &str, color: &str) {
    match color {
        "green" => println!("-----> {}", msg.green()),
        "yellow" => eprintln!(" !     {}", msg.yellow()),
        "red" => eprintln!(" !     {}", msg.red()),
        _ => println!("{}", msg),
    }
}
