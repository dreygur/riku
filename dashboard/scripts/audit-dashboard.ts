#!/usr/bin/env -S node --import tsx
/**
 * Standalone production audit for the riku dashboard's live backend routes.
 *
 * Real endpoints only — no fabricated routes:
 *   - GET  {DASHBOARD_URL}/api/health          (Hono catch-all, server/routers/supervisor.ts)
 *   - GET  {DASHBOARD_URL}/api/env/:app        (Hono catch-all, server/routers/env.ts)
 *   - GET  {DASHBOARD_URL}/api/metrics/apps    (Hono catch-all, used to build the worker matrix)
 *   - GET  {DASHBOARD_URL}/api/logs/stream     (app/api/logs/stream/route.ts, native SSE tail)
 *   - GET  {RIKU_API_URL}/metrics/stream       (Rust axum server, src/supervisor/health/mod.rs)
 *
 * NOTE: the dashboard has no Hono proxy for the metrics SSE stream — only
 * REST polling is proxied (/api/metrics, /api/metrics/apps). The live
 * "metrics-update" SSE events only exist on the Rust health server, so
 * Phase 2 connects there directly instead of pretending a passthrough
 * exists at /api/metrics.
 *
 * Run with Bun:  bun run scripts/audit-dashboard.ts
 * Run with Node: npx tsx scripts/audit-dashboard.ts
 */

import { appendFile, mkdir } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";

const DASHBOARD_URL = process.env.DASHBOARD_URL ?? "http://localhost:3000";
const RIKU_API_URL = process.env.RIKU_API_URL ?? "http://127.0.0.1:9091";
const AUDIT_APP = process.env.AUDIT_APP ?? "myapp";
const METRICS_WINDOW_MS = 5_000;
const LOG_LINE_COUNT = 10;
const LOG_WAIT_MS = 5_000;

const LOG_DIR = join(homedir(), ".riku", "logs", AUDIT_APP);
const LOG_FILE = join(LOG_DIR, "deploy.log");

// ── SSE frame parsing (works identically on Bun and Node — no EventSource dependency) ──

interface SseFrame {
  event: string | null;
  data: string;
}

async function consumeSse(
  url: string,
  durationMs: number,
  onFrame: (frame: SseFrame) => void,
): Promise<void> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), durationMs);

  try {
    const res = await fetch(url, { signal: controller.signal });
    if (!res.ok || !res.body) {
      throw new Error(`SSE connect failed: HTTP ${res.status}`);
    }

    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });

      let sepIndex: number;
      while ((sepIndex = buffer.indexOf("\n\n")) !== -1) {
        const rawFrame = buffer.slice(0, sepIndex);
        buffer = buffer.slice(sepIndex + 2);

        let event: string | null = null;
        const dataLines: string[] = [];
        for (const line of rawFrame.split("\n")) {
          if (line.startsWith("event:")) event = line.slice(6).trim();
          else if (line.startsWith("data:")) dataLines.push(line.slice(5).trim());
        }
        if (dataLines.length > 0) onFrame({ event, data: dataLines.join("\n") });
      }
    }
  } catch (e) {
    if (!controller.signal.aborted) throw e;
  } finally {
    clearTimeout(timer);
  }
}

// ── Phase 1: static handler verification ──

interface PhaseResult {
  ok: boolean;
  detail: string;
}

async function verifyStaticHandlers(): Promise<PhaseResult> {
  const healthRes = await fetch(`${DASHBOARD_URL}/api/health`);
  const healthCors = healthRes.headers.get("access-control-allow-origin");
  const health = await healthRes.json();
  const healthOk =
    healthRes.status === 200 &&
    healthCors === "*" &&
    typeof health.status === "string" &&
    typeof health.uptime === "number" &&
    typeof health.version === "string" &&
    typeof health.timestamp === "number";

  const envRes = await fetch(`${DASHBOARD_URL}/api/env/${AUDIT_APP}`);
  const env = await envRes.json();
  const envOk = envRes.status === 200 && env.app === AUDIT_APP && Array.isArray(env.vars);

  return {
    ok: healthOk && envOk,
    detail: `health=${healthRes.status}/cors=${healthCors} env=${envRes.status}`,
  };
}

// ── Phase 2: live metrics SSE consumption (direct to the Rust health server) ──

interface ProcessSample {
  pid: number | null;
  cpu_time_ms: number;
  memory_bytes: number;
  restart_count: number;
  status: string;
  app: string;
  kind: string;
  ordinal: number;
}

interface AppStatsFrame {
  app: string;
  processes: ProcessSample[];
}

interface MetricsAudit {
  ok: boolean;
  frameCount: number;
  firstFrame: { at: number; apps: AppStatsFrame[] } | null;
  lastFrame: { at: number; apps: AppStatsFrame[] } | null;
}

async function auditMetricsStream(): Promise<MetricsAudit> {
  const result: MetricsAudit = { ok: false, frameCount: 0, firstFrame: null, lastFrame: null };

  await consumeSse(`${RIKU_API_URL}/metrics/stream`, METRICS_WINDOW_MS, (frame) => {
    if (frame.event !== "metrics-update") return;
    let parsed: AppStatsFrame[];
    try {
      parsed = JSON.parse(frame.data);
    } catch {
      return;
    }
    if (!Array.isArray(parsed)) return;

    const valid = parsed.every((app) =>
      Array.isArray(app.processes) &&
      app.processes.every(
        (p: ProcessSample) =>
          (p.pid === null || typeof p.pid === "number") &&
          typeof p.cpu_time_ms === "number" &&
          typeof p.memory_bytes === "number",
      ),
    );
    if (!valid) return;

    result.frameCount += 1;
    const at = Date.now();
    if (!result.firstFrame) result.firstFrame = { at, apps: parsed };
    result.lastFrame = { at, apps: parsed };
  });

  result.ok = result.frameCount > 0;
  return result;
}

// ── Phase 3: log tail SSE round-trip through the real dashboard route ──

interface LogAudit {
  ok: boolean;
  written: number;
  captured: number;
  latenciesMs: number[];
}

async function auditLogTail(): Promise<LogAudit> {
  await mkdir(LOG_DIR, { recursive: true });

  const expected = new Map<string, number>(); // line -> write timestamp
  const captured = new Map<string, number>(); // line -> receive timestamp

  const consuming = consumeSse(
    `${DASHBOARD_URL}/api/logs/stream?app=${encodeURIComponent(AUDIT_APP)}`,
    LOG_WAIT_MS,
    (frame) => {
      try {
        const { line } = JSON.parse(frame.data) as { line: string };
        if (expected.has(line) && !captured.has(line)) {
          captured.set(line, Date.now());
        }
      } catch {
        // heartbeat comment lines have no `data:` and never reach here
      }
    },
  );

  // give the SSE connection a moment to attach before we start writing
  await new Promise((r) => setTimeout(r, 300));

  for (let i = 0; i < LOG_LINE_COUNT; i++) {
    const line = `AUDIT_LINE_${i}_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    expected.set(line, Date.now());
    await appendFile(LOG_FILE, line + "\n", "utf-8");
    await new Promise((r) => setTimeout(r, 25));
  }

  await consuming;

  const latenciesMs = [...expected.entries()]
    .filter(([line]) => captured.has(line))
    .map(([line, writtenAt]) => captured.get(line)! - writtenAt);

  return {
    ok: captured.size === LOG_LINE_COUNT,
    written: LOG_LINE_COUNT,
    captured: captured.size,
    latenciesMs,
  };
}

// ── Worker matrix (mirrors dashboard/lib/types.ts:mapBackendToWorkers) ──

interface WorkerRow {
  app: string;
  process: string;
  pid: number | null;
  rss: string;
  cpuPercent: string;
  restarts: number;
  status: string;
}

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const exp = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** exp;
  return `${value.toFixed(exp === 0 ? 0 : 1)} ${units[exp]}`;
}

/** Real instantaneous CPU% derived from two SSE samples: Δcpu_time_ms / Δwall_ms. */
function buildWorkerMatrix(metrics: MetricsAudit): WorkerRow[] {
  if (!metrics.firstFrame || !metrics.lastFrame) return [];
  const wallDeltaMs = metrics.lastFrame.at - metrics.firstFrame.at;

  const firstByKey = new Map<string, ProcessSample>();
  for (const app of metrics.firstFrame.apps) {
    for (const p of app.processes) firstByKey.set(`${p.app}:${p.kind}.${p.ordinal}`, p);
  }

  const rows: WorkerRow[] = [];
  for (const app of metrics.lastFrame.apps) {
    for (const p of app.processes) {
      const key = `${p.app}:${p.kind}.${p.ordinal}`;
      const prev = firstByKey.get(key);
      const cpuPercent =
        prev && wallDeltaMs > 0
          ? (((p.cpu_time_ms - prev.cpu_time_ms) / wallDeltaMs) * 100).toFixed(1) + "%"
          : "n/a";

      rows.push({
        app: p.app,
        process: `${p.kind}.${p.ordinal}`,
        pid: p.pid,
        rss: formatBytes(p.memory_bytes),
        cpuPercent,
        restarts: p.restart_count,
        status: p.status.toUpperCase(),
      });
    }
  }
  return rows;
}

// ── Slishee-style ASCII render ──

const WIDTH = 74;

function pad(s: string): string {
  return s.length >= WIDTH ? s.slice(0, WIDTH) : s + " ".repeat(WIDTH - s.length);
}

function line(content: string): string {
  return `│ ${pad(content)} │`;
}

function divider(): string {
  return `├${"─".repeat(WIDTH + 2)}┤`;
}

function render(
  staticResult: PhaseResult,
  metrics: MetricsAudit,
  logs: LogAudit,
  workers: WorkerRow[],
): string {
  const lines: string[] = [];
  lines.push(`┌${"─".repeat(WIDTH + 2)}┐`);
  lines.push(line("RIKU // DASHBOARD_AUDIT_DAEMON"));
  lines.push(divider());
  lines.push(line(`GATEWAY STATUS : [${staticResult.ok ? "200 OK" : "FAILED"}]`));
  lines.push(
    line(
      `METRICS SSE    : [${metrics.ok ? "STREAMING_ACTIVE" : "NO_FRAMES"}] (${metrics.frameCount} frames / ${METRICS_WINDOW_MS}ms)`,
    ),
  );
  lines.push(
    line(
      `LOG BUFFER     : [${logs.ok ? "TAILING_SUCCESS" : "INCOMPLETE"}] (${logs.captured}/${logs.written} lines)`,
    ),
  );
  if (logs.latenciesMs.length > 0) {
    const avg = (logs.latenciesMs.reduce((a, b) => a + b, 0) / logs.latenciesMs.length).toFixed(1);
    lines.push(line(`LOG LATENCY    : [avg ${avg}ms / max ${Math.max(...logs.latenciesMs)}ms]`));
  }
  lines.push(divider());
  lines.push(line("ACTIVE WORKER MATRIX:"));
  lines.push(
    line(
      `APP      │ PROC      │ PID    │ RSS MEM   │ CPU %  │ RESTARTS │ STATUS`,
    ),
  );
  if (workers.length === 0) {
    lines.push(line("(no processes reported by the metrics stream)"));
  } else {
    for (const w of workers) {
      lines.push(
        line(
          `${w.app.padEnd(8)} │ ${w.process.padEnd(9)} │ ${String(w.pid ?? "--").padEnd(6)} │ ${w.rss.padEnd(9)} │ ${w.cpuPercent.padEnd(6)} │ ${String(w.restarts).padEnd(8)} │ [${w.status}]`,
        ),
      );
    }
  }
  lines.push(`└${"─".repeat(WIDTH + 2)}┘`);
  return lines.join("\n");
}

// ── Orchestration ──

async function main(): Promise<void> {
  console.log("[1/3] verifying static handlers...");
  const staticResult = await verifyStaticHandlers();

  console.log("[2/3] consuming metrics SSE for 5s...");
  const metrics = await auditMetricsStream();

  console.log("[3/3] tailing deploy.log over SSE...");
  const logs = await auditLogTail();

  const workers = buildWorkerMatrix(metrics);

  console.clear();
  console.log(render(staticResult, metrics, logs, workers));

  const allOk = staticResult.ok && metrics.ok && logs.ok;
  process.exit(allOk ? 0 : 1);
}

main().catch((e) => {
  console.error("AUDIT FATAL:", e instanceof Error ? e.message : e);
  process.exit(1);
});
