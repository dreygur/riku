use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::collections::VecDeque;
use std::thread;
use std::time::Duration;

use crate::config::RikuPaths;
use crate::util::{display, exit_if_invalid};

/// Tail app log files using multi_tail.
pub fn cmd_logs(paths: &RikuPaths, app: &str, process: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let pattern = paths.log_root.join(&app).join(format!("{}.*.log", process));
    let logfiles: Vec<String> = glob::glob(pattern.to_str().unwrap_or(""))
        .map(|g| {
            g.filter_map(|e| e.ok().map(|p| p.to_string_lossy().to_string()))
                .collect()
        })
        .unwrap_or_default();

    if !logfiles.is_empty() {
        multi_tail(&logfiles)?;
    } else {
        display::warn(&format!("No logs found for app '{}'.", app));
    }
    Ok(())
}

/// Tail multiple log files, showing the last `catch_up` lines then polling.
fn multi_tail(filenames: &[String]) -> Result<()> {
    let catch_up: usize = 20;

    // Compute prefixes (filename stem without extension)
    let prefixes: Vec<String> = filenames
        .iter()
        .map(|f| {
            Path::new(f)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();

    let longest = prefixes.iter().map(|p| p.len()).max().unwrap_or(0);

    // Catch up: show last `catch_up` lines from each file
    for (i, f) in filenames.iter().enumerate() {
        if let Ok(file) = fs::File::open(f) {
            let reader = BufReader::new(file);
            #[allow(clippy::lines_filter_map_ok)]
            let lines: VecDeque<String> = reader
                .lines()
                .filter_map(Result::ok)
                .collect::<VecDeque<String>>();
            // Take last catch_up lines
            let start = if lines.len() > catch_up {
                lines.len() - catch_up
            } else {
                0
            };
            for line in lines.iter().skip(start) {
                println!(
                    "{}",
                    format!(
                        "{} | {}",
                        prefixes[i].as_str().to_string()
                            + &" ".repeat(longest.saturating_sub(prefixes[i].len())),
                        line
                    )
                    .white()
                );
            }
        }
    }

    // Open files at the end for tailing
    let mut files: Vec<fs::File> = Vec::new();
    let mut inodes: Vec<u64> = Vec::new();
    for f in filenames {
        let mut file = fs::File::open(f)?;
        let meta = file.metadata()?;
        inodes.push(meta.ino());
        file.seek(SeekFrom::End(0))?;
        files.push(file);
    }

    let mut active_filenames: Vec<String> = filenames.to_vec();

    loop {
        let mut updated = false;

        for i in 0..active_filenames.len() {
            let mut buf = String::new();
            if files[i].read_to_string(&mut buf).is_ok() && !buf.is_empty() {
                updated = true;
                for line in buf.lines() {
                    println!(
                        "{}",
                        format!("{:<width$} | {}", prefixes[i], line, width = longest).white()
                    );
                }
            }
        }

        if !updated {
            thread::sleep(Duration::from_secs(1));
            // Check for log rotation
            let mut i = 0;
            while i < active_filenames.len() {
                let f = &active_filenames[i];
                if Path::new(f).exists() {
                    if let Ok(meta) = fs::metadata(f) {
                        if meta.ino() != inodes[i] {
                            // Log rotated, reopen
                            if let Ok(mut new_file) = fs::File::open(f) {
                                let _ = new_file.seek(SeekFrom::Start(0));
                                files[i] = new_file;
                                inodes[i] = meta.ino();
                            }
                        }
                    }
                    i += 1;
                } else {
                    active_filenames.remove(i);
                    files.remove(i);
                    inodes.remove(i);
                    // Don't increment i since we removed an element
                }
            }
            if active_filenames.is_empty() {
                break;
            }
        }
    }

    Ok(())
}
