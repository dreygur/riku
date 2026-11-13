//! CPU time and memory of a worker's whole process tree.
//!
//! A Procfile worker runs as `sh -c <cmd>`, so its pid is the shell and the
//! real app is a child of it. A worker without a cgroup of its own is
//! measured by summing every process under its pid, from one snapshot of
//! `/proc` per supervisor tick. Other platforms have no `/proc` and read an
//! empty snapshot, so their workers report zero.

// Project rule 16 forbids `?`, so every early return here is spelled out.
#![allow(clippy::question_mark)]

/// CPU time and resident memory of a worker.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResourceUsage {
    pub cpu_time_ms: u64,
    pub memory_bytes: u64,
}

impl std::ops::AddAssign for ResourceUsage {
    fn add_assign(&mut self, other: ResourceUsage) {
        self.cpu_time_ms += other.cpu_time_ms;
        self.memory_bytes += other.memory_bytes;
    }
}

/// One process of the snapshot: its parent and its own usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcEntry {
    pub pid: u32,
    pub ppid: u32,
    pub usage: ResourceUsage,
}

/// The fields riku reads from one `/proc/<pid>/stat` line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatFields {
    pub ppid: u32,
    /// `utime + stime + cutime + cstime` in clock ticks. The children terms
    /// hold the time of reaped children, which `/proc` no longer lists.
    pub cpu_ticks: u64,
    pub rss_pages: u64,
}

/// Parses a `/proc/<pid>/stat` line. The command name sits in parentheses
/// and may itself hold spaces or parentheses, so fields are counted from
/// the last `)`.
pub fn parse_stat(line: &str) -> Option<StatFields> {
    let Some(close) = line.rfind(')') else {
        return None;
    };
    let fields: Vec<&str> = line[close + 1..].split_whitespace().collect();
    if fields.len() < 22 {
        return None;
    }
    let number = |index: usize| fields[index].parse::<u64>().ok();
    let ppid = match fields[1].parse::<u32>() {
        Ok(ppid) => ppid,
        Err(_) => return None,
    };
    match (number(11), number(12), number(13), number(14), number(21)) {
        (Some(utime), Some(stime), Some(cutime), Some(cstime), Some(rss_pages)) => {
            Some(StatFields {
                ppid,
                cpu_ticks: utime + stime + cutime + cstime,
                rss_pages,
            })
        }
        _ => None,
    }
}

/// A snapshot of every process on the host.
pub struct ProcessTable {
    entries: Vec<ProcEntry>,
}

impl ProcessTable {
    pub fn from_entries(entries: Vec<ProcEntry>) -> ProcessTable {
        ProcessTable { entries }
    }

    /// Usage of `root` plus every process below it. A pid gone from the
    /// snapshot adds nothing.
    pub fn tree_usage(&self, root: u32) -> ResourceUsage {
        let mut members = vec![root];
        let mut total = ResourceUsage::default();
        let mut next = 0;
        while next < members.len() {
            let pid = members[next];
            next += 1;
            for entry in &self.entries {
                if entry.pid == pid {
                    total += entry.usage;
                } else if entry.ppid == pid && !members.contains(&entry.pid) {
                    members.push(entry.pid);
                }
            }
        }
        total
    }

    /// Reads `/proc/<pid>/stat` for every process. A process that exits
    /// during the read is left out.
    #[cfg(target_os = "linux")]
    pub fn read() -> ProcessTable {
        let dir = match std::fs::read_dir("/proc") {
            Ok(dir) => dir,
            Err(_) => return ProcessTable::from_entries(Vec::new()),
        };
        let units = linux::Units::of_host();
        let entries = dir
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_str().and_then(|n| n.parse().ok()))
            .filter_map(|pid| linux::read_entry(pid, &units))
            .collect();
        ProcessTable::from_entries(entries)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn read() -> ProcessTable {
        ProcessTable::from_entries(Vec::new())
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{parse_stat, ProcEntry, ResourceUsage};

    /// Clock ticks per second and bytes per page, to turn `/proc` counts
    /// into milliseconds and bytes.
    pub(super) struct Units {
        ticks_per_second: u64,
        page_bytes: u64,
    }

    impl Units {
        pub(super) fn of_host() -> Units {
            // SAFETY: sysconf only reads a system constant.
            let (ticks, page) = unsafe {
                (
                    libc::sysconf(libc::_SC_CLK_TCK),
                    libc::sysconf(libc::_SC_PAGESIZE),
                )
            };
            Units {
                ticks_per_second: u64::try_from(ticks).unwrap_or(100).max(1),
                page_bytes: u64::try_from(page).unwrap_or(4096),
            }
        }
    }

    pub(super) fn read_entry(pid: u32, units: &Units) -> Option<ProcEntry> {
        let line = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(line) => line,
            Err(_) => return None,
        };
        let Some(fields) = parse_stat(&line) else {
            return None;
        };
        Some(ProcEntry {
            pid,
            ppid: fields.ppid,
            usage: ResourceUsage {
                cpu_time_ms: fields.cpu_ticks * 1000 / units.ticks_per_second,
                memory_bytes: fields.rss_pages * units.page_bytes,
            },
        })
    }
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;
