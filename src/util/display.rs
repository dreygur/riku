//! Terminal display utilities: colored output and table formatting.
//!
//! ## Standard `after_help` format for CLI commands
//!
//! ```text
//! #[command(after_help = "Examples:\n  riku <cmd> arg1\n  riku <cmd> arg1 --flag")]
//! ```
//!
//! ## Output function guide
//!
//! | Function    | Prefix / style          | Stream | Use for                               |
//! |-------------|-------------------------|--------|---------------------------------------|
//! | `info`      | `"-----> "` green       | stdout | deployment steps, status              |
//! | `success`   | `"✓ "` green bold       | stdout | completed actions                     |
//! | `warn`      | `" !     "` yellow      | stderr | non-fatal issues, hints               |
//! | `error`     | `" !     "` red bold    | stderr | fatal errors (follow with bail!/Err)  |
//! | `step`      | `"       "` cyan        | stdout | sub-steps within an operation         |
//! | `note`      | `"       "` white       | stdout | data / config display                 |
//! | `section`   | `"=== ... ==="` green bold | stdout | section headers                    |
//! | `kv`        | `"  key   value"`       | stdout | key-value pairs                       |
//! | `blank`     | empty line              | stdout | visual separator                      |
//! | `echo`      | Heroku-style colored    | varies | deploy pipeline (keep existing usage) |

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

/// `"-----> msg"` green stdout — deployment steps, status.
pub fn info(msg: &str) {
    println!("{} {}", "----->".green(), msg);
}

/// `"✓ msg"` green bold stdout — completed actions.
pub fn success(msg: &str) {
    println!("{} {}", "✓".green().bold(), msg);
}

/// `" !     msg"` yellow stderr — warnings, non-fatal issues.
pub fn warn(msg: &str) {
    eprintln!("{} {}", " !    ".yellow(), msg);
}

/// `" !     msg"` red bold stderr — errors (follow with `bail!` or `return Err`).
pub fn error(msg: &str) {
    eprintln!("{} {}", " !    ".red().bold(), msg);
}

/// `"       msg"` cyan stdout — sub-steps, indented progress.
pub fn step(msg: &str) {
    println!("       {}", msg.cyan());
}

/// `"       msg"` white stdout — data/config display.
pub fn note(msg: &str) {
    println!("       {}", msg.white());
}

/// `"=== title ==="` green bold stdout — section headers.
pub fn section(title: &str) {
    println!("{}", format!("=== {} ===", title).green().bold());
}

/// `"  key   value"` stdout — key-value pair, key is bold.
pub fn kv(key: &str, value: &str) {
    println!("  {:<20} {}", key.bold(), value);
}

/// Print a blank separator line.
pub fn blank() {
    println!();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_table_empty_headers_returns_empty() {
        let result = format_table(&[], &[], 2);
        assert_eq!(result, "");
    }

    #[test]
    fn test_format_table_single_column_no_rows() {
        let result = format_table(&["Name"], &[], 2);
        assert!(result.contains("Name"));
        assert!(result.contains("----"));
    }

    #[test]
    fn test_format_table_header_separator_present() {
        let result = format_table(&["App", "Status"], &[], 2);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("---"));
    }

    #[test]
    fn test_format_table_with_rows() {
        let rows = vec![
            vec!["myapp".to_string(), "running".to_string()],
            vec!["other".to_string(), "stopped".to_string()],
        ];
        let result = format_table(&["App", "Status"], &rows, 2);
        assert!(result.contains("myapp"));
        assert!(result.contains("running"));
        assert!(result.contains("other"));
        assert!(result.contains("stopped"));
    }

    #[test]
    fn test_format_table_column_width_expands_to_longest_cell() {
        let rows = vec![vec!["a-very-long-app-name".to_string(), "ok".to_string()]];
        let result = format_table(&["App", "St"], &rows, 2);
        // "App" column must be at least 20 chars wide (padded)
        let first_data_line = result.lines().nth(2).unwrap();
        assert!(first_data_line.starts_with("a-very-long-app-name"));
    }

    #[test]
    fn test_format_table_column_spacing_applied() {
        let rows = vec![vec!["app".to_string(), "ok".to_string()]];
        let result_2 = format_table(&["App", "St"], &rows, 2);
        let result_4 = format_table(&["App", "St"], &rows, 4);
        // Wider spacing means more spaces between columns
        assert!(result_4.len() > result_2.len());
    }

    #[test]
    fn test_format_table_row_with_fewer_cells_than_headers() {
        // Should not panic when a row has fewer cells than headers
        let rows = vec![vec!["only-one".to_string()]];
        let result = format_table(&["App", "Status"], &rows, 2);
        assert!(result.contains("only-one"));
    }
}
