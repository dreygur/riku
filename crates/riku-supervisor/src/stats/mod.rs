//! Process statistics and metrics module.
//!
//! Tracks process health, resource usage, and performance metrics.

pub mod manager;
pub mod queries;
pub mod resources;
pub mod types;

pub use manager::StatsManager;
pub use resources::get_process_resources;
pub use types::{AppStats, HealthStatus, ProcessStatus};

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
