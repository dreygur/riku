//! Plain column tables for listing commands (`apps`, `ps`, `stats`). Tables
//! stay outside the rail: they are data, not progress.

use super::rail::emit;
use super::style::{paint, Paint, Stream};

/// Build a formatted table from headers and rows.
pub fn format_table(headers: &[&str], rows: &[Vec<String>], column_spacing: usize) -> String {
    if headers.is_empty() {
        return String::new();
    }

    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    let spacing = " ".repeat(column_spacing);
    let mut output = padded_row(headers, &widths, &spacing);
    let separator: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    output.push_str(&separator.join(&spacing));
    output.push('\n');
    for row in rows {
        let cells: Vec<&str> = row.iter().map(String::as_str).collect();
        output.push_str(&padded_row(&cells, &widths, &spacing));
    }
    output
}

fn padded_row(cells: &[&str], widths: &[usize], spacing: &str) -> String {
    let padded: Vec<String> = cells
        .iter()
        .enumerate()
        .map(|(i, cell)| format!("{:<width$}", cell, width = widths.get(i).unwrap_or(&0)))
        .collect();
    format!("{}\n", padded.join(spacing))
}

/// Print a table.
pub fn print_table(headers: &[&str], rows: &[Vec<String>], column_spacing: usize) {
    emit(Stream::Stdout, &format_table(headers, rows, column_spacing));
}

/// Print a table under a bold title line.
pub fn print_table_with_title(
    title: &str,
    headers: &[&str],
    rows: &[Vec<String>],
    column_spacing: usize,
) {
    emit(
        Stream::Stdout,
        &paint(title, Paint::Bold, Stream::Stdout.colour()),
    );
    emit(Stream::Stdout, "");
    print_table(headers, rows, column_spacing);
}

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
