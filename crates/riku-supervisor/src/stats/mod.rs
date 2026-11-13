//! Process statistics and metrics module.
//!
//! Tracks process health, resource usage, and performance metrics.

pub mod manager;
pub mod queries;
pub mod repository;
pub mod resources;
pub mod stats_file;
pub mod types;

pub use manager::StatsManager;
pub use repository::read_stats_file;
pub use resources::{ProcessTable, ResourceUsage};
pub use stats_file::replace_app_container_rows;
pub use types::{AppStats, ContainerFacts, HealthStatus, ProcessStats, ProcessStatus};

#[cfg(test)]
mod tests {
    use super::types::ProcessStats;
    use super::*;

    fn process_stats(manager: &StatsManager, process_id: &str) -> ProcessStats {
        manager
            .get_all_stats()
            .into_iter()
            .flat_map(|app| app.processes)
            .find(|p| p.process_id == process_id)
            .unwrap_or_else(|| panic!("no stats tracked for {}", process_id))
    }

    fn app_stats(manager: &StatsManager, app: &str) -> AppStats {
        manager
            .get_all_stats()
            .into_iter()
            .find(|a| a.app == app)
            .unwrap_or_else(|| panic!("no stats tracked for app {}", app))
    }

    #[test]
    fn test_stats_manager_creation() {
        let manager = StatsManager::new();
        assert!(
            manager.get_all_stats().is_empty(),
            "a fresh manager must track no apps"
        );
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

        let stats = process_stats(&manager, "app-web-1");
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

        let stats = process_stats(&manager, "app-web-1");
        assert_eq!(stats.status, ProcessStatus::Running);
        assert_eq!(stats.pid, Some(12345));
    }

    #[test]
    fn restart_count_survives_respawn() {
        let mut manager = StatsManager::new();
        let register = |m: &mut StatsManager| {
            m.register_process(
                "app-web-1".to_string(),
                "app".to_string(),
                "web".to_string(),
                1,
            )
        };
        register(&mut manager);
        manager.mark_restarting("app-web-1");
        register(&mut manager);

        let stats = process_stats(&manager, "app-web-1");
        assert_eq!(stats.restart_count, 1, "respawn must not reset the count");
        assert!(stats.last_restart_at.is_some());
        assert_eq!(stats.status, ProcessStatus::Starting);
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

        let stats = process_stats(&manager, "app-web-1");
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

        let stats = app_stats(&manager, "app");
        assert_eq!(stats.total_processes, 2);
        assert_eq!(stats.running_processes, 2);
        assert_eq!(stats.healthy_processes, 1);
    }
}
