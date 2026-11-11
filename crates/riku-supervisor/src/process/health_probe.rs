//! The HTTP health probe of one managed process.

use std::time::Duration;

use crate::config::HealthCheck;
use crate::stats::HealthStatus;

use super::ProcessManager;

impl ProcessManager {
    /// Perform an HTTP health check on a process.
    pub(super) fn perform_health_check(
        &self,
        process_id: &str,
        config: &HealthCheck,
    ) -> HealthStatus {
        use reqwest::blocking::Client;

        let port = self.get_process_port(process_id);
        let url = match port {
            Some(p) => format!("http://127.0.0.1:{}{}", p, config.url),
            None => return HealthStatus::Error("No port configured".to_string()),
        };

        let client = riku_http::blocking_client_builder()
            .timeout(Duration::from_secs(config.timeout))
            .build()
            .unwrap_or_else(|_| Client::new());

        match client.get(&url).send() {
            Ok(response) => {
                if response.status().is_success() {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Unhealthy
                }
            }
            Err(e) => {
                if e.is_timeout() {
                    HealthStatus::Timeout
                } else {
                    HealthStatus::Error(e.to_string())
                }
            }
        }
    }

    /// Get the port for a process from its environment.
    pub(super) fn get_process_port(&self, process_id: &str) -> Option<u16> {
        self.processes
            .get(process_id)
            .and_then(|p| p.config.env.get("PORT"))
            .and_then(|p: &String| p.parse::<u16>().ok())
    }
}
