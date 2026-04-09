//! Log tailing commands for deployed apps.

use anyhow::Result;
use colored::Colorize;
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::config::RikuPaths;
use crate::util::{display, exit_if_invalid, validate_app_name};

/// Number of existing lines to print per file before entering tail mode.
const CATCH_UP_LINES: usize = 20;

/// How long to sleep between poll iterations when no new data arrives.
const TAIL_POLL_INTERVAL: Duration = Duration::from_secs(1);

// ---------------------------------------------------------------------------
// Public commands
// ---------------------------------------------------------------------------

/// Show the persistent deploy log for an app.
///
/// When `follow` is true, the log is tailed live (polling every 200 ms) until
/// Ctrl-C terminates the process. Only the contents written since the file was
/// last truncated (i.e. the most recent deploy) are shown.
pub fn cmd_deploy_logs(paths: &RikuPaths, app: &str, follow: bool) -> Result<()> {
    let app = validate_app_name(app)?;
    let log_file = paths.deploy_log_file(&app);

    if !log_file.exists() {
        display::warn(&format!(
            "No deploy log found for '{}'. Deploy the app first.",
            app
        ));
        return Ok(());
    }

    display::section(&format!("Deploy log: {}", app));

    if follow {
        tail_deploy_log(&log_file)?;
    } else {
        for line in fs::read_to_string(&log_file)?.lines() {
            println!("{}", line);
        }
    }

    Ok(())
}

/// Tail all process log files for an app, multiplexed with a filename prefix.
pub fn cmd_logs(paths: &RikuPaths, app: &str, process: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let pattern = paths.log_root.join(&app).join(format!("{}.*.log", process));
    let logfiles: Vec<String> = glob::glob(pattern.to_str().unwrap_or(""))
        .map(|g| {
            g.filter_map(|e| e.ok().map(|p| p.to_string_lossy().to_string()))
                .collect()
        })
        .unwrap_or_default();

    if logfiles.is_empty() {
        display::warn(&format!("No logs found for app '{}'.", app));
    } else {
        multi_tail(&logfiles)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Print existing content then poll for new lines until Ctrl-C.
fn tail_deploy_log(log_file: &Path) -> Result<()> {
    let mut file = fs::File::open(log_file)?;
    let mut initial = String::new();
    file.read_to_string(&mut initial)?;
    for line in initial.lines() {
        println!("{}", line);
    }
    loop {
        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_ok() && !buf.is_empty() {
            for line in buf.lines() {
                println!("{}", line);
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
}

/// Multiplex-tail multiple log files with aligned filename prefixes.
///
/// Shows the last [`CATCH_UP_LINES`] lines from each file, then enters a
/// poll loop. Handles log rotation by comparing inodes on each idle cycle.
/// Exits when all tracked files have been deleted.
fn multi_tail(filenames: &[String]) -> Result<()> {
    let prefixes: Vec<String> = filenames.iter().map(stem_prefix).collect();
    let col_width = prefixes.iter().map(|p| p.len()).max().unwrap_or(0);

    print_catch_up(filenames, &prefixes, col_width);

    let (mut files, mut inodes) = open_at_end(filenames)?;
    let mut active: Vec<String> = filenames.to_vec();

    loop {
        if drain_new_lines(&mut files, &prefixes, col_width) {
            continue;
        }

        thread::sleep(TAIL_POLL_INTERVAL);
        reopen_rotated(&active, &mut files, &mut inodes);
        remove_deleted(&mut active, &mut files, &mut inodes);

        if active.is_empty() {
            break;
        }
    }

    Ok(())
}

/// Derive a display prefix from a file path's stem (e.g. `web.1` from `web.1.log`).
fn stem_prefix(path: &String) -> String {
    Path::new(path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

/// Print the last [`CATCH_UP_LINES`] lines from each file to stdout.
fn print_catch_up(filenames: &[String], prefixes: &[String], col_width: usize) {
    for (i, f) in filenames.iter().enumerate() {
        let Ok(file) = fs::File::open(f) else { continue };
        let reader = BufReader::new(file);
        #[allow(clippy::lines_filter_map_ok)]
        let lines: VecDeque<String> = reader.lines().filter_map(Result::ok).collect();
        let start = lines.len().saturating_sub(CATCH_UP_LINES);
        for line in lines.iter().skip(start) {
            print_prefixed(&prefixes[i], col_width, line);
        }
    }
}

/// Open every file, seek to EOF, and return the file handles alongside their inodes.
fn open_at_end(filenames: &[String]) -> Result<(Vec<fs::File>, Vec<u64>)> {
    let mut files = Vec::with_capacity(filenames.len());
    let mut inodes = Vec::with_capacity(filenames.len());
    for f in filenames {
        let mut file = fs::File::open(f)?;
        inodes.push(file.metadata()?.ino());
        file.seek(SeekFrom::End(0))?;
        files.push(file);
    }
    Ok((files, inodes))
}

/// Read and print any new data from each open file.
///
/// Returns `true` if at least one file had new content (skip the sleep).
fn drain_new_lines(
    files: &mut [fs::File],
    prefixes: &[String],
    col_width: usize,
) -> bool {
    let mut had_output = false;
    for (i, file) in files.iter_mut().enumerate() {
        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_ok() && !buf.is_empty() {
            had_output = true;
            for line in buf.lines() {
                print_prefixed(&prefixes[i], col_width, line);
            }
        }
    }
    had_output
}

/// Reopen any file whose inode has changed (log rotation).
fn reopen_rotated(active: &[String], files: &mut [fs::File], inodes: &mut [u64]) {
    for (i, path) in active.iter().enumerate() {
        let Ok(meta) = fs::metadata(path) else { continue };
        if meta.ino() == inodes[i] {
            continue;
        }
        if let Ok(mut new_file) = fs::File::open(path) {
            let _ = new_file.seek(SeekFrom::Start(0));
            files[i] = new_file;
            inodes[i] = meta.ino();
        }
    }
}

/// Remove entries for log files that no longer exist on disk.
fn remove_deleted(
    active: &mut Vec<String>,
    files: &mut Vec<fs::File>,
    inodes: &mut Vec<u64>,
) {
    let mut i = 0;
    while i < active.len() {
        if Path::new(&active[i]).exists() {
            i += 1;
        } else {
            active.remove(i);
            files.remove(i);
            inodes.remove(i);
        }
    }
}

/// Print a single log line with a left-aligned filename prefix.
fn print_prefixed(prefix: &str, col_width: usize, line: &str) {
    println!(
        "{}",
        format!("{:<width$} | {}", prefix, line, width = col_width).white()
    );
}
