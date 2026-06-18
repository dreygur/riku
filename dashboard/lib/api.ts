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

export type ProcessStats = {
  process_id: string;
  app: string;
  kind: string;
  ordinal: number;
  pid: number | null;
  status: string;
  started_at: string;
  last_health_check: string | null;
  health_check_status: string;
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
    getApp: (app: string) =>
      apiFetch<AppStats[]>(`/metrics/apps/${encodeURIComponent(app)}`),
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
};
