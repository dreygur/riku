// Shared view-model types for the riku dashboard frontend. These mirror the
// real data shapes surfaced by the supervisor, deploy, and nginx subsystems
// (see src/supervisor/, src/deploy/, src/nginx.rs in the riku source tree).

import type { AppStats, NetworkEntry, ProcessStats } from "@/lib/api";

export type ProcessStatus = "running" | "stopped" | "crashed";

export type HealthState = "healthy" | "unhealthy" | "unknown";

/** One worker process under supervision, as reported per app. */
export interface WorkerInfo {
  app: string;
  /** Worker name from the Procfile, e.g. "web", "worker", "cron". */
  process: string;
  pid: number | null;
  /** Resident set size, in bytes. */
  rssBytes: number;
  /** Cumulative CPU time consumed, in milliseconds. */
  cpuTimeMs: number;
  status: ProcessStatus;
  health: HealthState;
  restartCount: number;
}

/** A single timestamped line from a live deploy log stream. */
export interface DeployLogLine {
  timestamp: string;
  line: string;
}

/**
 * Per-app network/TLS configuration, read directly from the generated nginx
 * config and cert files by app/api/network/route.ts (no riku runtime API for
 * this exists — see that route's comment).
 */
export interface NetworkInfo {
  app: string;
  serverName: string | null;
  /** Upstream proxy_pass target, e.g. "http://127.0.0.1:5000". */
  upstream: string | null;
  /** TLS cert expiry (from the cert itself), or null if unprovisioned. */
  tlsExpiry: string | null;
}

/** Identity mapping today, kept as a mapper for parity with the other
 * backend-shape -> view-model conversions in this file, and so the route's
 * response shape can drift independently of the dashboard's view-model. */
export function mapBackendToNetwork(entries: NetworkEntry[]): NetworkInfo[] {
  return entries.map((e) => ({
    app: e.app,
    serverName: e.serverName,
    upstream: e.upstream,
    tlsExpiry: e.tlsExpiry,
  }));
}

/** A single environment variable row as edited in the env editor. */
export interface EnvVar {
  key: string;
  value: string;
}

// ---------------------------------------------------------------------------
// Mapping helpers — convert raw backend shapes into dashboard view-models.
// The raw types come from lib/api.ts which mirrors the Rust supervisor
// health server at 127.0.0.1:9091.
// ---------------------------------------------------------------------------

const STATUS_MAP: Record<string, ProcessStatus> = {
  running: "running",
  stopped: "stopped",
  crashed: "crashed",
  starting: "running",
  restarting: "running",
};

const HEALTH_MAP: Record<string, HealthState> = {
  healthy: "healthy",
  unhealthy: "unhealthy",
  unknown: "unknown",
  timeout: "unhealthy",
  error: "unhealthy",
};

/** Flatten all processes across all apps into a single WorkerInfo array. */
export function mapBackendToWorkers(apps: AppStats[]): WorkerInfo[] {
  const workers: WorkerInfo[] = [];
  for (const app of apps) {
    for (const p of app.processes) {
      workers.push({
        app: p.app,
        process: `${p.kind}.${p.ordinal}`,
        pid: p.pid,
        rssBytes: p.memory_bytes,
        cpuTimeMs: p.cpu_time_ms,
        status: STATUS_MAP[p.status] ?? "stopped",
        health: HEALTH_MAP[p.health_check_status] ?? "unknown",
        restartCount: p.restart_count,
      });
    }
  }
  return workers;
}
