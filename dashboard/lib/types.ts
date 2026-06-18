// Shared view-model types for the riku dashboard frontend. These mirror the
// real data shapes surfaced by the supervisor, deploy, and nginx subsystems
// (see src/supervisor/, src/deploy/, src/nginx.rs in the riku source tree).

import type {
  AppStats,
  HealthCheckStatus,
  NetworkEntry,
  ProcessStats,
} from "@/lib/api";

export type ProcessStatus =
  | "running"
  | "stopped"
  | "crashed"
  | "starting"
  | "restarting"
  | "oom_killed";

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
  /** Failure detail when health is "unhealthy" due to a probe error, e.g. "connection refused". */
  healthDetail: string | null;
  restartCount: number;
  /** When this process instance was started, or null if never started. */
  startedAt: string | null;
  /** When the most recent health probe ran, or null if none yet. */
  lastHealthCheck: string | null;
  /** When the process was last restarted, or null if never restarted. */
  lastRestartAt: string | null;
  /** Total requests served since this process started. */
  requestsTotal: number;
  /** Current request throughput, requests/sec. */
  requestsPerSecond: number;
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
  starting: "starting",
  restarting: "restarting",
  oom_killed: "oom_killed",
};

/** Unpack the `HealthCheckStatus` wire shape into a coarse UI state plus an
 * optional human-readable detail (only set for the `{error: "..."}` shape). */
function mapHealth(raw: HealthCheckStatus): {
  health: HealthState;
  detail: string | null;
} {
  if (typeof raw === "object" && raw !== null) {
    return { health: "unhealthy", detail: raw.error };
  }
  switch (raw) {
    case "healthy":
      return { health: "healthy", detail: null };
    case "unhealthy":
      return { health: "unhealthy", detail: null };
    case "timeout":
      return { health: "unhealthy", detail: "probe timed out" };
    case "unknown":
    default:
      return { health: "unknown", detail: null };
  }
}

/** Flatten all processes across all apps into a single WorkerInfo array. */
export function mapBackendToWorkers(apps: AppStats[]): WorkerInfo[] {
  const workers: WorkerInfo[] = [];
  for (const app of apps) {
    for (const p of app.processes) {
      const { health, detail } = mapHealth(p.health_check_status);
      workers.push({
        app: p.app,
        process: `${p.kind}.${p.ordinal}`,
        pid: p.pid,
        rssBytes: p.memory_bytes,
        cpuTimeMs: p.cpu_time_ms,
        status: STATUS_MAP[p.status] ?? "stopped",
        health,
        healthDetail: detail,
        restartCount: p.restart_count,
        startedAt: p.started_at,
        lastHealthCheck: p.last_health_check,
        lastRestartAt: p.last_restart_at,
        requestsTotal: p.requests_total,
        requestsPerSecond: p.requests_per_second,
      });
    }
  }
  return workers;
}
