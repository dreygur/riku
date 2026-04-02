//! Process resource usage via the /proc filesystem (Linux) or stubs for other platforms.

/// Get process resource usage from /proc filesystem (Linux only).
#[cfg(target_os = "linux")]
pub fn get_process_resources(pid: u32) -> Option<(u64, u64)> {
    use std::fs;
    use std::io::Read;

    // Get memory usage from /proc/[pid]/status
    let status_path = format!("/proc/{}/status", pid);
    let mut memory_bytes = 0u64;

    if let Ok(mut file) = fs::File::open(&status_path) {
        let mut content = String::new();
        if file.read_to_string(&mut content).is_ok() {
            for line in content.lines() {
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<u64>() {
                            memory_bytes = kb * 1024; // Convert KB to bytes
                        }
                    }
                    break;
                }
            }
        }
    }

    // Get CPU time from /proc/[pid]/stat
    let stat_path = format!("/proc/{}/stat", pid);
    let mut cpu_time_ms = 0u64;

    if let Ok(mut file) = fs::File::open(&stat_path) {
        let mut content = String::new();
        if file.read_to_string(&mut content).is_ok() {
            let parts: Vec<&str> = content.split_whitespace().collect();
            if parts.len() > 14 {
                if let Ok(utime) = parts[13].parse::<u64>() {
                    if let Some(stime) = parts.get(14).and_then(|s| s.parse::<u64>().ok()) {
                        // Convert clock ticks to milliseconds (assuming 100 Hz clock)
                        cpu_time_ms = (utime + stime) * 1000 / 100;
                    }
                }
            }
        }
    }

    Some((cpu_time_ms, memory_bytes))
}

#[cfg(not(target_os = "linux"))]
pub fn get_process_resources(_pid: u32) -> Option<(u64, u64)> {
    // Return default values for non-Linux systems
    Some((0, 0))
}
