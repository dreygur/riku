import { Hono } from "hono";
import { dashboardToken, isSameOrigin } from "../security";

const RIKU_API = process.env.RIKU_API_URL ?? "http://127.0.0.1:9091";
const UPSTREAM_TIMEOUT_MS = 5_000;

const OFFLINE_METRICS_PAYLOAD = [
  {
    app: "system",
    total_processes: 0,
    running_processes: 0,
    healthy_processes: 0,
    total_restarts: 0,
    total_memory_bytes: 0,
    total_cpu_time_ms: 0,
    processes: [],
    last_updated: new Date().toISOString(),
  },
];

function offlineAppMetrics(appName: string) {
  return {
    app: appName,
    total_processes: 0,
    running_processes: 0,
    healthy_processes: 0,
    total_restarts: 0,
    total_memory_bytes: 0,
    total_cpu_time_ms: 0,
    processes: [],
    last_updated: new Date().toISOString(),
  };
}

// ── Fetch upstream with timeout; never throws — caller gets ok:false on any failure ──
async function safeUpstreamFetch(path: string): Promise<{
  ok: boolean;
  body: string | null;
  error: string | null;
}> {
  try {
    const res = await fetch(`${RIKU_API}${path}`, {
      signal: AbortSignal.timeout(UPSTREAM_TIMEOUT_MS),
    });
    if (!res.ok) {
      return { ok: false, body: null, error: `upstream returned ${res.status}` };
    }
    return { ok: true, body: await res.text(), error: null };
  } catch (e) {
    return { ok: false, body: null, error: e instanceof Error ? e.message : String(e) };
  }
}

export const supervisorRouter = new Hono();

// ── GET /csrf ── Deliver the operator token to the dashboard's own JS ──
//
// Token-delivery approach (see also server/security.ts): the dashboard is the
// trusted origin, so its own client JS fetches the operator token here and
// echoes it in `x-riku-dashboard-token` on mutating calls. This endpoint is
// gated by the same same-origin check as mutating routes, so a cross-site page
// cannot read the token (CORS already blocks reading the response body, and the
// Origin/Sec-Fetch-Site check blocks the request outright). The token is never
// shipped in the JS bundle and never exposed via NEXT_PUBLIC_*.
supervisorRouter.get("/csrf", (c) => {
  if (!isSameOrigin(c)) {
    return c.json({ ok: false, error: "cross-site origin rejected" }, 403);
  }
  const token = dashboardToken();
  if (!token) {
    return c.json({ ok: false, error: "dashboard token not configured" }, 503);
  }
  return c.json({ token });
});

// ── GET /metrics ── Proxy to riku native metrics endpoint ──
supervisorRouter.get("/metrics", async (c) => {
  const result = await safeUpstreamFetch("/metrics");
  if (!result.ok) return c.json(OFFLINE_METRICS_PAYLOAD);

  try {
    return c.json(JSON.parse(result.body!));
  } catch {
    return c.json(OFFLINE_METRICS_PAYLOAD);
  }
});

// ── GET /metrics/apps ── Proxy to riku metrics/apps ──
supervisorRouter.get("/metrics/apps", async (c) => {
  const result = await safeUpstreamFetch("/metrics/apps");
  if (!result.ok) return c.json(OFFLINE_METRICS_PAYLOAD);

  try {
    return c.json(JSON.parse(result.body!));
  } catch {
    return c.json(OFFLINE_METRICS_PAYLOAD);
  }
});

// ── GET /metrics/apps/:app ── Proxy to riku metrics/apps/:app ──
supervisorRouter.get("/metrics/apps/:app", async (c) => {
  const { app: appName } = c.req.param();
  const result = await safeUpstreamFetch(`/metrics/apps/${encodeURIComponent(appName)}`);
  if (!result.ok) return c.json(offlineAppMetrics(appName));

  try {
    return c.json(JSON.parse(result.body!));
  } catch {
    return c.json(offlineAppMetrics(appName));
  }
});

// ── GET /plugins ── Proxy client plugin listing ──
supervisorRouter.get("/plugins", async (c) => {
  const result = await safeUpstreamFetch("/plugins");
  if (!result.ok) return c.json({ plugins: [] });

  try {
    return c.json(JSON.parse(result.body!));
  } catch {
    return c.json({ plugins: [] });
  }
});

// ── GET /hooks ── Proxy server-side hook plugin listing ──
supervisorRouter.get("/hooks", async (c) => {
  const result = await safeUpstreamFetch("/hooks");
  if (!result.ok) return c.json({ hooks: [] });

  try {
    return c.json(JSON.parse(result.body!));
  } catch {
    return c.json({ hooks: [] });
  }
});

// ── GET /health ── Proxy health check; never returns a 502 ──
supervisorRouter.get("/health", async (c) => {
  const result = await safeUpstreamFetch("/health");
  const offline = {
    status: "unreachable",
    uptime: 0,
    version: "unknown",
    timestamp: Math.floor(Date.now() / 1000),
  };
  if (!result.ok) return c.json(offline);

  try {
    return c.json(JSON.parse(result.body!));
  } catch {
    return c.json(offline);
  }
});
