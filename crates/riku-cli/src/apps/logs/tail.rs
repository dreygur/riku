//! Log tailing engine supporting multi-file follow and rotation.

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use chrono::Local;
use colored::Colorize;

use crate::util::log_stamp::split_stamp;

const CATCH_UP_LINES: usize = 20;
const TAIL_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Print existing content then poll for new lines until Ctrl-C.
#[allow(clippy::question_mark)]
pub(super) fn tail_deploy_log(log_file: &Path) -> Result<()> {
    let mut file = match fs::File::open(log_file) {
        Ok(f) => f,
        Err(e) => return Err(e.into()),
    };
    let mut initial = String::new();
    match file.read_to_string(&mut initial) {
        Ok(_) => {}
        Err(e) => return Err(e.into()),
    };
    for line in initial.lines() {
        println!("{line}");
    }
    loop {
        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_ok() && !buf.is_empty() {
            for line in buf.lines() {
                println!("{line}");
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
}

/// Multiplex-tail multiple log files with aligned filename prefixes.
#[allow(clippy::question_mark)]
pub(super) fn multi_tail(filenames: &[String]) -> Result<()> {
    let prefixes: Vec<String> = filenames.iter().map(stem_prefix).collect();
    let col_width = prefixes.iter().map(|p| p.len()).max().unwrap_or(0);

    print_catch_up(filenames, &prefixes, col_width);

    let (mut files, mut inodes) = match open_at_end(filenames) {
        Ok(pair) => pair,
        Err(e) => return Err(e),
    };
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

fn stem_prefix(path: &String) -> String {
    Path::new(path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn print_catch_up(filenames: &[String], prefixes: &[String], col_width: usize) {
    for (i, f) in filenames.iter().enumerate() {
        let Ok(file) = fs::File::open(f) else {
            continue;
        };
        let reader = BufReader::new(file);
        #[allow(clippy::lines_filter_map_ok)]
        let lines: VecDeque<String> = reader.lines().filter_map(Result::ok).collect();
        let start = lines.len().saturating_sub(CATCH_UP_LINES);
        for line in lines.iter().skip(start) {
            print_prefixed(&prefixes[i], col_width, line);
        }
    }
}

#[allow(clippy::question_mark)]
fn open_at_end(filenames: &[String]) -> Result<(Vec<fs::File>, Vec<u64>)> {
    let mut files = Vec::with_capacity(filenames.len());
    let mut inodes = Vec::with_capacity(filenames.len());
    for f in filenames {
        let mut file = match fs::File::open(f) {
            Ok(file) => file,
            Err(e) => return Err(e.into()),
        };
        let metadata = match file.metadata() {
            Ok(m) => m,
            Err(e) => return Err(e.into()),
        };
        inodes.push(metadata.ino());
        match file.seek(SeekFrom::End(0)) {
            Ok(_) => {}
            Err(e) => return Err(e.into()),
        };
        files.push(file);
    }
    Ok((files, inodes))
}

fn drain_new_lines(files: &mut [fs::File], prefixes: &[String], col_width: usize) -> bool {
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

fn reopen_rotated(active: &[String], files: &mut [fs::File], inodes: &mut [u64]) {
    for (i, path) in active.iter().enumerate() {
        let Ok(meta) = fs::metadata(path) else {
            continue;
        };
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

fn remove_deleted(active: &mut Vec<String>, files: &mut Vec<fs::File>, inodes: &mut Vec<u64>) {
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

/// Width of the `HH:MM:SS` capture time, kept as blanks for a line with no
/// stamp so the columns stay aligned.
const CLOCK_WIDTH: usize = 8;

/// `line` behind its local capture time (see `riku_util::log_stamp`) and
/// the worker prefix padded to `col_width`.
fn prefixed(prefix: &str, col_width: usize, line: &str) -> String {
    let (clock, text) = match split_stamp(line) {
        Some((at, text)) => (at.with_timezone(&Local).format("%H:%M:%S").to_string(), text),
        None => (" ".repeat(CLOCK_WIDTH), line),
    };
    format!("{clock} {prefix:<col_width$} | {text}")
}

fn print_prefixed(prefix: &str, col_width: usize, line: &str) {
    println!("{}", prefixed(prefix, col_width, line).white());
}

#[cfg(test)]
#[path = "tail_tests.rs"]
mod tests;
