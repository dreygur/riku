//! Resource usage of running containers.
//!
//! Security model: engine output is untrusted. Every number is parsed and
//! range checked, and a missing or malformed field is an error naming the
//! field. Ids passed to the engine are [`ContainerId`]s, and an empty id list
//! never runs the engine, because `stats` with no ids would report the whole
//! host.

use std::ffi::OsString;

use anyhow::{anyhow, Result};
use serde_json::Value;

use super::snapshot::fetch_present;
use super::{ContainerId, Engine};

/// One sample of a container's resource use.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerStats {
    /// The full container id when the engine reports it, else the short id.
    pub id: String,
    /// CPU use as a percentage of one core, as the engine prints it.
    pub cpu_percent: f64,
    /// Memory in use, in bytes.
    pub memory_bytes: u64,
}

const MEMORY_UNITS: [(&str, f64); 7] = [
    ("B", 1.0),
    ("kB", 1e3),
    ("KiB", 1024.0),
    ("MB", 1e6),
    ("MiB", 1_048_576.0),
    ("GB", 1e9),
    ("GiB", 1_073_741_824.0),
];

fn text_field<'a>(entry: &'a Value, keys: &[&str]) -> Result<&'a str> {
    for key in keys {
        if let Some(text) = entry[*key].as_str() {
            return Ok(text);
        }
    }
    Err(anyhow!("stats entry has no {}", keys.join(" or ")))
}

fn parse_cpu(raw: &str) -> Result<f64> {
    match raw.trim().trim_end_matches('%').parse::<f64>() {
        Ok(value) if value.is_finite() && value >= 0.0 => Ok(value),
        _ => Err(anyhow!("CPUPerc {raw:?} is not a percentage")),
    }
}

fn parse_memory(usage: &str) -> Result<u64> {
    let used = usage
        .split_once(" / ")
        .map_or(usage, |(used, _)| used)
        .trim();
    let split = used
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(used.len());
    let (number, unit) = used.split_at(split);
    let value = match number.parse::<f64>() {
        Ok(value) if value.is_finite() => value,
        _ => return Err(anyhow!("MemUsage {usage:?} does not start with a number")),
    };
    match MEMORY_UNITS.iter().find(|(name, _)| *name == unit) {
        Some((_, factor)) => Ok((value * factor).round() as u64),
        None => Err(anyhow!("MemUsage {usage:?} has an unknown unit {unit:?}")),
    }
}

fn parse_entry(entry: &Value) -> Result<ContainerStats> {
    let id = match text_field(entry, &["Container", "ID", "id"]) {
        Ok(id) => id,
        Err(e) => return Err(anyhow!("unusable stats entry: {e}")),
    };
    let cpu = match text_field(entry, &["CPUPerc", "cpu_percent"]) {
        Ok(raw) => parse_cpu(raw),
        Err(e) => Err(e),
    };
    let memory = match text_field(entry, &["MemUsage", "mem_usage"]) {
        Ok(raw) => parse_memory(raw),
        Err(e) => Err(e),
    };
    match (cpu, memory) {
        (Ok(cpu_percent), Ok(memory_bytes)) => Ok(ContainerStats {
            id: id.to_string(),
            cpu_percent,
            memory_bytes,
        }),
        (Err(e), _) | (_, Err(e)) => Err(anyhow!("stats for {id}: {e}")),
    }
}

fn parse_entries(output: &str) -> Result<Vec<Value>> {
    let trimmed = output.trim();
    if trimmed.starts_with('[') {
        return match serde_json::from_str(trimmed) {
            Ok(entries) => Ok(entries),
            Err(e) => Err(anyhow!("engine stats output is not valid JSON: {e}")),
        };
    }
    trimmed
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| match serde_json::from_str(line) {
            Ok(entry) => Ok(entry),
            Err(e) => Err(anyhow!("engine stats line is not valid JSON: {e}")),
        })
        .collect()
}

/// Read `stats --no-stream --format json` output: one JSON object per line
/// (docker) or one JSON array (podman).
pub(crate) fn parse_stats(output: &str) -> Result<Vec<ContainerStats>> {
    parse_entries(output).and_then(|entries| entries.iter().map(parse_entry).collect())
}

impl Engine {
    /// One sample for each container in `ids`.
    ///
    /// Returns an empty list without running the engine when `ids` is empty.
    /// Pass running containers: the engine cannot sample a stopped one. A
    /// container removed after it was listed is left out of the result; any
    /// other failure is an error. See [`super::snapshot`] for how the two are
    /// told apart.
    pub fn stats(&self, ids: &[ContainerId]) -> Result<Vec<ContainerStats>> {
        let run = |ids: &[ContainerId]| {
            let mut args: Vec<OsString> = ["stats", "--no-stream", "--format", "json"]
                .into_iter()
                .map(OsString::from)
                .collect();
            args.extend(ids.iter().map(|id| OsString::from(id.as_str())));
            self.output(&args)
        };
        match fetch_present(self, ids, run, parse_stats) {
            Ok(stats) => Ok(stats),
            Err(e) => Err(anyhow!("cannot read container stats: {e}")),
        }
    }
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod tests;
