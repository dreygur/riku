//! What a worker's cgroup reports about everything inside it.

use std::fs;

use super::WorkerCgroup;
use crate::stats::resources::ResourceUsage;

impl WorkerCgroup {
    /// Memory and CPU use of every process in the cgroup, from
    /// `memory.current` and the `usage_usec` line of `cpu.stat`. `None` when
    /// either cannot be read, as when the memory controller is not
    /// delegated.
    pub fn usage(&self) -> Option<ResourceUsage> {
        let memory = fs::read_to_string(self.path.join("memory.current"));
        let cpu_stat = fs::read_to_string(self.path.join("cpu.stat"));
        let (Ok(memory), Ok(cpu_stat)) = (memory, cpu_stat) else {
            return None;
        };
        let usage_usec = cpu_stat
            .lines()
            .find_map(|line| line.strip_prefix("usage_usec "))
            .and_then(|value| value.trim().parse::<u64>().ok());
        match (memory.trim().parse::<u64>(), usage_usec) {
            (Ok(memory_bytes), Some(usec)) => Some(ResourceUsage {
                cpu_time_ms: usec / 1000,
                memory_bytes,
            }),
            _ => None,
        }
    }

    /// The `memory.max` limit in bytes; `None` when it is `max` (no limit)
    /// or cannot be read.
    pub fn memory_max(&self) -> Option<u64> {
        match fs::read_to_string(self.path.join("memory.max")) {
            Ok(text) => text.trim().parse().ok(),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
#[path = "usage_tests.rs"]
mod tests;
