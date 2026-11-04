//! Container statistics mapping to supervisor ProcessStats rows.

use std::collections::HashMap;

use riku_container::engine::{ContainerInfo, ContainerState, ContainerStats};
use riku_util::file_names::worker::WorkerName;

use super::tally::RestartTally;
use crate::stats::types::{ContainerFacts, HealthStatus, ProcessStats, ProcessStatus};

fn find_sample<'a>(
    container_id: &str,
    samples: &'a HashMap<String, ContainerStats>,
) -> Option<&'a ContainerStats> {
    if let Some(sample) = samples.get(container_id) {
        return Some(sample);
    }
    for (sample_id, sample) in samples {
        if container_id.starts_with(sample_id) {
            return Some(sample);
        }
    }
    None
}

fn map_state_and_exit(state: &ContainerState) -> (String, Option<i32>) {
    match state {
        ContainerState::Running => ("running".to_string(), None),
        ContainerState::Starting => ("starting".to_string(), None),
        ContainerState::Unhealthy => ("unhealthy".to_string(), None),
        ContainerState::Restarting => ("restarting".to_string(), None),
        ContainerState::Exited { code } => ("exited".to_string(), Some(*code)),
        ContainerState::Created => ("created".to_string(), None),
        ContainerState::Other(text) => (text.to_lowercase(), None),
    }
}

fn map_status(state: &ContainerState) -> ProcessStatus {
    match state {
        ContainerState::Running | ContainerState::Starting | ContainerState::Unhealthy => {
            ProcessStatus::Running
        }
        ContainerState::Created => ProcessStatus::Starting,
        ContainerState::Restarting => ProcessStatus::Restarting,
        ContainerState::Exited { code } => {
            if *code == 0 {
                ProcessStatus::Stopped
            } else {
                ProcessStatus::Crashed
            }
        }
        ContainerState::Other(_) => ProcessStatus::Stopped,
    }
}

fn map_health_status(state: &ContainerState) -> HealthStatus {
    match state {
        ContainerState::Unhealthy => HealthStatus::Unhealthy,
        _ => HealthStatus::Unknown,
    }
}

/// Map container inspection and sample data into ProcessStats rows.
pub(crate) fn container_rows(
    infos: &[ContainerInfo],
    samples: &HashMap<String, ContainerStats>,
    tally: &RestartTally,
) -> Vec<ProcessStats> {
    let mut rows = Vec::new();
    for info in infos {
        let app = match &info.app {
            Some(a) => a,
            None => continue,
        };
        let service = match &info.service {
            Some(s) => s,
            None => continue,
        };
        let copy = match &info.copy {
            Some(c) => c,
            None => continue,
        };

        let sample = find_sample(info.id.as_str(), samples);
        let memory_bytes = match sample {
            Some(s) => s.memory_bytes,
            None => 0,
        };
        let cpu_percent = match sample {
            Some(s) => s.cpu_percent,
            None => 0.0,
        };

        let (state_str, exit_code) = map_state_and_exit(&info.state);
        let facts = ContainerFacts {
            id: info.id.as_str().to_string(),
            image: info.image.clone(),
            state: state_str,
            exit_code,
            cpu_percent,
        };

        rows.push(ProcessStats {
            process_id: WorkerName::new(app, service.as_str(), copy.get()).to_string(),
            app: app.clone(),
            kind: service.as_str().to_string(),
            ordinal: copy.get(),
            pid: None,
            status: map_status(&info.state),
            started_at: None,
            last_health_check: None,
            health_check_status: map_health_status(&info.state),
            restart_count: tally.count(&info.name),
            last_restart_at: None,
            cpu_time_ms: 0,
            memory_bytes,
            requests_total: 0,
            requests_per_second: 0.0,
            container: Some(facts),
        });
    }
    rows
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod tests;
