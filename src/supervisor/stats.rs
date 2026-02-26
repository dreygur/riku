//! Process statistics and metrics module.
//!
//! Tracks process health, resource usage, and performance metrics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::time::Instant;

/// Statistics for a single process.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessStats {
    pub process_id: String,
    pub app: String,
    pub kind: String,
    pub ordinal: u32,
    pub pid: Option<u32>,
    pub status: ProcessStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub last_health_check: Option<DateTime<Utc>>,
    pub health_check_status: HealthStatus,
    pub restart_count: u32,
    pub last_restart_at: Option<DateTime<Utc>>,
    pub cpu_time_ms: u64,
    pub memory_bytes: u64,
    pub requests_total: u64,
    pub requests_per_second: f64,
}

impl Default for ProcessStats {
    fn default() -> Self {
        ProcessStats {
            process_id: String::new(),
            app: String::new(),
            kind: String::new(),
            ordinal: 0,
            pid: None,
            status: ProcessStatus::Starting,
            started_at: None,
            last_health_check: None,
            health_check_status: HealthStatus::Unknown,
            restart_count: 0,
            last_restart_at: None,
            cpu_time_ms: 0,
            memory_bytes: 0,
            requests_total: 0,
            requests_per_second: 0.0,
        }
    }
}

/// Process status enum.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ProcessStatus {
    Starting,
    Running,
    Stopped,
    Crashed,
    Restarting,
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessStatus::Starting => write!(f, "starting"),
            ProcessStatus::Running => write!(f, "running"),
            ProcessStatus::Stopped => write!(f, "stopped"),
            ProcessStatus::Crashed => write!(f, "crashed"),
            ProcessStatus::Restarting => write!(f, "restarting"),
        }
    }
}

/// Health check status.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum HealthStatus {
    Unknown,
    Healthy,
    Unhealthy,
    Timeout,
    Error(String),
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Unknown => write!(f, "unknown"),
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
            HealthStatus::Timeout => write!(f, "timeout"),
            HealthStatus::Error(e) => write!(f, "error: {}", e),
        }
    }
}

/// Aggregated statistics for an application.
#[allow(dead_code)]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppStats {
    pub app: String,
    pub total_processes: u32,
    pub running_processes: u32,
    pub healthy_processes: u32,
    pub total_restarts: u32,
    pub total_memory_bytes: u64,
    pub total_cpu_time_ms: u64,
    pub processes: Vec<ProcessStats>,
    pub last_updated: DateTime<Utc>,
}

/// Statistics manager for tracking all process metrics.
pub struct StatsManager {
    stats: HashMap<String, ProcessStats>,
    start_times: HashMap<String, Instant>,
    #[allow(dead_code)]
    request_counts: HashMap<String, u64>,
}

impl StatsManager {
    /// Create a new stats manager.
    pub fn new() -> Self {
        StatsManager {
            stats: HashMap::new(),
            start_times: HashMap::new(),
            request_counts: HashMap::new(),
        }
    }

    /// Register a new process.
    pub fn register_process(
        &mut self,
        process_id: String,
        app: String,
        kind: String,
        ordinal: u32,
    ) {
        let stats = ProcessStats {
            process_id: process_id.clone(),
            app,
            kind,
            ordinal,
            started_at: Some(Utc::now()),
            status: ProcessStatus::Starting,
            ..Default::default()
        };

        self.stats.insert(process_id.clone(), stats);
        self.start_times.insert(process_id, Instant::now());
    }

    /// Mark a process as running.
    pub fn mark_running(&mut self, process_id: &str, pid: u32) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.status = ProcessStatus::Running;
            stats.pid = Some(pid);
        }
    }

    /// Mark a process as stopped.
    pub fn mark_stopped(&mut self, process_id: &str) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.status = ProcessStatus::Stopped;
        }
    }

    /// Mark a process as crashed.
    pub fn mark_crashed(&mut self, process_id: &str) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.status = ProcessStatus::Crashed;
        }
    }

    /// Mark a process as restarting.
    pub fn mark_restarting(&mut self, process_id: &str) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.status = ProcessStatus::Restarting;
            stats.restart_count += 1;
            stats.last_restart_at = Some(Utc::now());
        }
    }

    /// Update health check status.
    pub fn update_health_check(&mut self, process_id: &str, status: HealthStatus) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.health_check_status = status;
            stats.last_health_check = Some(Utc::now());
        }
    }

    /// Update resource usage for a process.
    pub fn update_resource_usage(&mut self, process_id: &str, cpu_ms: u64, memory_bytes: u64) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.cpu_time_ms = cpu_ms;
            stats.memory_bytes = memory_bytes;
        }
    }

    /// Increment request count for a process.
    #[allow(dead_code)]
    pub fn increment_requests(&mut self, process_id: &str) {
        if let Some(stats) = self.stats.get_mut(process_id) {
            stats.requests_total += 1;

            // Calculate requests per second
            if let Some(start_time) = self.start_times.get(process_id) {
                let elapsed = start_time.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    stats.requests_per_second = stats.requests_total as f64 / elapsed;
                }
            }
        }

        *self
            .request_counts
            .entry(process_id.to_string())
            .or_insert(0) += 1;
    }

    /// Get stats for a specific process.
    #[allow(dead_code)]
    pub fn get_process_stats(&self, process_id: &str) -> Option<&ProcessStats> {
        self.stats.get(process_id)
    }

    /// Get stats for all processes of an app.
    #[allow(dead_code)]
    pub fn get_app_stats(&self, app: &str) -> AppStats {
        let processes: Vec<ProcessStats> = self
            .stats
            .values()
            .filter(|s| s.app == app)
            .cloned()
            .collect();

        let total_processes = processes.len() as u32;
        let running_processes = processes
            .iter()
            .filter(|p| p.status == ProcessStatus::Running)
            .count() as u32;
        let healthy_processes = processes
            .iter()
            .filter(|p| p.health_check_status == HealthStatus::Healthy)
            .count() as u32;
        let total_restarts = processes.iter().map(|p| p.restart_count).sum();
        let total_memory_bytes = processes.iter().map(|p| p.memory_bytes).sum();
        let total_cpu_time_ms = processes.iter().map(|p| p.cpu_time_ms).sum();

        AppStats {
            app: app.to_string(),
            total_processes,
            running_processes,
            healthy_processes,
            total_restarts,
            total_memory_bytes,
            total_cpu_time_ms,
            processes,
            last_updated: Utc::now(),
        }
    }

    /// Get stats for all apps.
    #[allow(dead_code)]
    pub fn get_all_stats(&self) -> Vec<AppStats> {
        let mut apps: HashMap<String, AppStats> = HashMap::new();

        for stats in self.stats.values() {
            let app_stats = apps.entry(stats.app.clone()).or_insert_with(|| AppStats {
                app: stats.app.clone(),
                total_processes: 0,
                running_processes: 0,
                healthy_processes: 0,
                total_restarts: 0,
                total_memory_bytes: 0,
                total_cpu_time_ms: 0,
                processes: Vec::new(),
                last_updated: Utc::now(),
            });

            app_stats.total_processes += 1;
            if stats.status == ProcessStatus::Running {
                app_stats.running_processes += 1;
            }
            if stats.health_check_status == HealthStatus::Healthy {
                app_stats.healthy_processes += 1;
            }
            app_stats.total_restarts += stats.restart_count;
            app_stats.total_memory_bytes += stats.memory_bytes;
            app_stats.total_cpu_time_ms += stats.cpu_time_ms;
            app_stats.processes.push(stats.clone());
        }

        apps.into_values().collect()
    }

    /// Remove stats for a process.
    #[allow(dead_code)]
    pub fn remove_process(&mut self, process_id: &str) {
        self.stats.remove(process_id);
        self.start_times.remove(process_id);
        self.request_counts.remove(process_id);
    }

    /// Get total memory usage across all processes.
    #[allow(dead_code)]
    pub fn total_memory_usage(&self) -> u64 {
        self.stats.values().map(|s| s.memory_bytes).sum()
    }

    /// Get total process count.
    #[allow(dead_code)]
    pub fn total_processes(&self) -> usize {
        self.stats.len()
    }

    /// Get running process count.
    #[allow(dead_code)]
    pub fn running_processes(&self) -> usize {
        self.stats
            .values()
            .filter(|s| s.status == ProcessStatus::Running)
            .count()
    }

    /// Write stats to a JSON file for CLI consumption.
    pub fn write_stats_to_file(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let app_stats = self.get_all_stats();
        let json = serde_json::to_string_pretty(&app_stats)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        fs::write(path, json)?;
        Ok(())
    }
}

/// Get process resource usage from /proc filesystem (Linux only).
#[cfg(target_os = "linux")]
pub fn get_process_resources(pid: u32) -> Option<(u64, u64)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_manager_creation() {
        let manager = StatsManager::new();
        assert_eq!(manager.total_processes(), 0);
    }

    #[test]
    fn test_register_process() {
        let mut manager = StatsManager::new();
        manager.register_process(
            "app-web-1".to_string(),
            "app".to_string(),
            "web".to_string(),
            1,
        );

        let stats = manager.get_process_stats("app-web-1");
        assert!(stats.is_some());
        let stats = stats.unwrap();
        assert_eq!(stats.app, "app");
        assert_eq!(stats.kind, "web");
        assert_eq!(stats.status, ProcessStatus::Starting);
    }

    #[test]
    fn test_mark_running() {
        let mut manager = StatsManager::new();
        manager.register_process(
            "app-web-1".to_string(),
            "app".to_string(),
            "web".to_string(),
            1,
        );
        manager.mark_running("app-web-1", 12345);

        let stats = manager.get_process_stats("app-web-1").unwrap();
        assert_eq!(stats.status, ProcessStatus::Running);
        assert_eq!(stats.pid, Some(12345));
    }

    #[test]
    fn test_health_check_update() {
        let mut manager = StatsManager::new();
        manager.register_process(
            "app-web-1".to_string(),
            "app".to_string(),
            "web".to_string(),
            1,
        );
        manager.update_health_check("app-web-1", HealthStatus::Healthy);

        let stats = manager.get_process_stats("app-web-1").unwrap();
        assert_eq!(stats.health_check_status, HealthStatus::Healthy);
        assert!(stats.last_health_check.is_some());
    }

    #[test]
    fn test_app_stats() {
        let mut manager = StatsManager::new();
        manager.register_process(
            "app-web-1".to_string(),
            "app".to_string(),
            "web".to_string(),
            1,
        );
        manager.register_process(
            "app-web-2".to_string(),
            "app".to_string(),
            "web".to_string(),
            2,
        );
        manager.mark_running("app-web-1", 12345);
        manager.mark_running("app-web-2", 12346);
        manager.update_health_check("app-web-1", HealthStatus::Healthy);

        let app_stats = manager.get_app_stats("app");
        assert_eq!(app_stats.total_processes, 2);
        assert_eq!(app_stats.running_processes, 2);
        assert_eq!(app_stats.healthy_processes, 1);
    }
}
