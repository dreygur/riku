// All requests go through the Next.js API proxy at /api/*,
// which forwards to the riku supervisor on the server side.

// ── Backend response types (mirror the Rust supervisor health server) ──

export type AppStats = {
  app: string;
  total_processes: number;
  running_processes: number;
  healthy_processes: number;
  total_restarts: number;
  total_memory_bytes: number;
  total_cpu_time_ms: number;
  processes: ProcessStats[];
  last_updated: string;
};

/** The Rust `HealthStatus` enum's `Error(String)` variant serializes as
 * `{"error": "<message>"}` rather than a plain string (serde's default
 * external tagging for a tuple variant) — every other variant is a string. */
export type HealthCheckStatus =
  | "unknown"
  | "healthy"
  | "unhealthy"
  | "timeout"
  | { error: string };

export type ProcessStats = {
  process_id: string;
  app: string;
  kind: string;
  ordinal: number;
  pid: number | null;
  /** Includes "oom_killed" when the kernel OOM killer terminated the worker. */
  status: string;
  started_at: string | null;
  last_health_check: string | null;
  health_check_status: HealthCheckStatus;
  restart_count: number;
  last_restart_at: string | null;
  cpu_time_ms: number;
  memory_bytes: number;
  requests_total: number;
  requests_per_second: number;
};

export type HealthResponse = {
  status: string;
  uptime: number;
  version: string;
  timestamp: number;
};

export type EnvResponse = {
  app: string;
  vars: EnvVarEntry[];
};

export type EnvVarEntry = {
  key: string;
  value: string;
};

export type NetworkEntry = {
  app: string;
  serverName: string | null;
  upstream: string | null;
  tlsExpiry: string | null;
};

export type NetworkResponse = {
  apps: NetworkEntry[];
};

// ── Typed fetch helpers ──

async function apiFetch<T>(
  path: string,
  init?: RequestInit,
): Promise<T> {
  const res = await fetch(`/api${path}`, init);
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    const msg = body?.error ?? res.statusText;
    throw new Error(`HTTP ${res.status}: ${msg}`);
  }
  return res.json() as Promise<T>;
}

export const api = {
  health: {
    get: () => apiFetch<HealthResponse>("/health"),
  },

  metrics: {
    get: () => apiFetch<AppStats[]>("/metrics"),
    getApps: () => apiFetch<AppStats[]>("/metrics/apps"),
    /** Unlike `get`/`getApps`, the backend returns a single object here,
     * not an array (see src/supervisor/health/mod.rs metrics_app_handler). */
    getApp: (app: string) =>
      apiFetch<AppStats>(`/metrics/apps/${encodeURIComponent(app)}`),
  },

  network: {
    list: () => apiFetch<NetworkResponse>("/network"),
  },

  plugins: {
    list: () => apiFetch<{ plugins: string[] }>("/plugins"),
  },

  hooks: {
    list: () => apiFetch<{ hooks: string[] }>("/hooks"),
  },

  env: {
    list: (app: string) =>
      apiFetch<EnvResponse>(`/env/${encodeURIComponent(app)}`),
    set: (app: string, key: string, value: string) =>
      apiFetch<{ ok: boolean }>(`/env/${encodeURIComponent(app)}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ key, value }),
      }),
    delete: (app: string, key: string) =>
      apiFetch<{ ok: boolean }>(`/env/${encodeURIComponent(app)}`, {
        method: "DELETE",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ key }),
      }),
  },

  control: {
    create: (name: string) =>
      apiFetch<ControlActionResponse>("/control/apps", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name }),
      }),
    deploy: (app: string) =>
      apiFetch<ControlActionResponse>(
        `/control/apps/${encodeURIComponent(app)}/deploy`,
        { method: "POST" },
      ),
    restart: (app: string) =>
      apiFetch<ControlActionResponse>(
        `/control/apps/${encodeURIComponent(app)}/restart`,
        { method: "POST" },
      ),
    stop: (app: string) =>
      apiFetch<ControlActionResponse>(
        `/control/apps/${encodeURIComponent(app)}/stop`,
        { method: "POST" },
      ),
    destroy: (app: string) =>
      apiFetch<ControlActionResponse>(
        `/control/apps/${encodeURIComponent(app)}`,
        { method: "DELETE" },
      ),
    installPlugins: (only?: string[]) =>
      apiFetch<ControlActionResponse>("/control/plugins/install", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(only ? { only } : {}),
      }),
    containerExport: (app: string) =>
      apiFetch<ControlActionResponse & { output?: string }>(
        `/control/apps/${encodeURIComponent(app)}/container/export`,
        { method: "POST" },
      ),
  },
};

export type ControlActionResponse = {
  ok: boolean;
  app?: string;
  action?: string;
  error?: string;
};
