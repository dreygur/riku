import { Hono } from "hono";
import type { ContentfulStatusCode } from "hono/utils/http-status";
import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { requireMutatingAuth } from "../security";

// Mirrors src/supervisor/health/control.rs — mutating app-lifecycle actions
// gated by the shared-secret control token. The token never leaves this
// server: the browser only ever talks to /api/control/*, and this router
// attaches `Authorization: Bearer <token>` to the upstream Rust request.
const RIKU_API = process.env.RIKU_API_URL ?? "http://127.0.0.1:9091";
const UPSTREAM_TIMEOUT_MS = 30_000; // deploy/destroy can take longer than read routes
const CONTAINER_BUILD_TIMEOUT_MS = 600_000; // docker/podman build can run for minutes
const RIKU_ROOT = process.env.RIKU_ROOT ?? join(homedir(), ".riku");
const CONTROL_TOKEN_FILE = join(RIKU_ROOT, "control.token");

async function readControlToken(): Promise<string> {
  const raw = await readFile(CONTROL_TOKEN_FILE, "utf-8");
  const token = raw.trim();
  if (!token) {
    throw new Error(`control token file is empty: ${CONTROL_TOKEN_FILE}`);
  }
  return token;
}

interface UpstreamResult {
  ok: boolean;
  status: number;
  body: unknown;
  error: string | null;
}

async function controlFetch(
  path: string,
  method: "POST" | "DELETE",
  jsonBody?: unknown,
  timeoutMs: number = UPSTREAM_TIMEOUT_MS,
): Promise<UpstreamResult> {
  let token: string;
  try {
    token = await readControlToken();
  } catch (e) {
    return {
      ok: false,
      status: 503,
      body: null,
      error: `control token unavailable: ${e instanceof Error ? e.message : String(e)}`,
    };
  }

  try {
    const res = await fetch(`${RIKU_API}${path}`, {
      method,
      signal: AbortSignal.timeout(timeoutMs),
      headers: {
        Authorization: `Bearer ${token}`,
        ...(jsonBody !== undefined ? { "Content-Type": "application/json" } : {}),
      },
      body: jsonBody !== undefined ? JSON.stringify(jsonBody) : undefined,
    });

    const text = await res.text();
    let body: unknown = null;
    try {
      body = text ? JSON.parse(text) : null;
    } catch {
      body = text;
    }

    return { ok: res.ok, status: res.status, body, error: res.ok ? null : "upstream rejected request" };
  } catch (e) {
    return {
      ok: false,
      status: 502,
      body: null,
      error: e instanceof Error ? e.message : String(e),
    };
  }
}

export const controlRouter = new Hono();

// Every control route mutates app lifecycle state — gate the whole group.
controlRouter.use("/control/*", requireMutatingAuth);

// ── POST /control/apps ── Create a new app — body: { name } ──
controlRouter.post("/control/apps", async (c) => {
  const body = await c.req
    .json<{ name?: string }>()
    .catch(() => ({ name: undefined }) as { name?: string });
  if (!body.name || typeof body.name !== "string") {
    return c.json({ ok: false, error: "missing required field 'name'" }, 400);
  }

  const result = await controlFetch("/control/apps", "POST", { name: body.name });
  return c.json(result.body ?? { ok: result.ok, error: result.error }, result.status as ContentfulStatusCode);
});

// ── POST /control/apps/:app/deploy ──
controlRouter.post("/control/apps/:app/deploy", async (c) => {
  const { app } = c.req.param();
  const result = await controlFetch(`/control/apps/${encodeURIComponent(app)}/deploy`, "POST");
  return c.json(result.body ?? { ok: result.ok, error: result.error }, result.status as ContentfulStatusCode);
});

// ── POST /control/apps/:app/restart ──
controlRouter.post("/control/apps/:app/restart", async (c) => {
  const { app } = c.req.param();
  const result = await controlFetch(`/control/apps/${encodeURIComponent(app)}/restart`, "POST");
  return c.json(result.body ?? { ok: result.ok, error: result.error }, result.status as ContentfulStatusCode);
});

// ── POST /control/apps/:app/stop ──
controlRouter.post("/control/apps/:app/stop", async (c) => {
  const { app } = c.req.param();
  const result = await controlFetch(`/control/apps/${encodeURIComponent(app)}/stop`, "POST");
  return c.json(result.body ?? { ok: result.ok, error: result.error }, result.status as ContentfulStatusCode);
});

// ── DELETE /control/apps/:app ──
controlRouter.delete("/control/apps/:app", async (c) => {
  const { app } = c.req.param();
  const result = await controlFetch(`/control/apps/${encodeURIComponent(app)}`, "DELETE");
  return c.json(result.body ?? { ok: result.ok, error: result.error }, result.status as ContentfulStatusCode);
});

// ── POST /control/plugins/install ── body: { only?: string[] } ──
controlRouter.post("/control/plugins/install", async (c) => {
  const body = await c.req
    .json<{ only?: string[] }>()
    .catch(() => ({ only: undefined }) as { only?: string[] });

  const result = await controlFetch(
    "/control/plugins/install",
    "POST",
    body.only ? { only: body.only } : {},
  );
  return c.json(result.body ?? { ok: result.ok, error: result.error }, result.status as ContentfulStatusCode);
});

// ── POST /control/apps/:app/container/export ── server-local docker/podman build ──
controlRouter.post("/control/apps/:app/container/export", async (c) => {
  const { app } = c.req.param();
  const result = await controlFetch(
    `/control/apps/${encodeURIComponent(app)}/container/export`,
    "POST",
    undefined,
    CONTAINER_BUILD_TIMEOUT_MS,
  );
  return c.json(result.body ?? { ok: result.ok, error: result.error }, result.status as ContentfulStatusCode);
});
