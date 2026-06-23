import { Hono } from "hono";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";
import { requireMutatingAuth } from "../security";

const ENVS_ROOT = join(homedir(), ".riku", "envs");

function envFilePath(appName: string): string {
  return join(ENVS_ROOT, appName, "ENV");
}

function parseEnvFile(content: string): Record<string, string> {
  const vars: Record<string, string> = {};
  for (const line of content.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eqIdx = trimmed.indexOf("=");
    if (eqIdx === -1) continue;
    const key = trimmed.slice(0, eqIdx).trim();
    const value = trimmed.slice(eqIdx + 1).trim();
    if (key) vars[key] = value;
  }
  return vars;
}

function serializeEnvFile(vars: Record<string, string>): string {
  return Object.entries(vars).map(([k, v]) => `${k}=${v}`).join("\n") + "\n";
}

async function readVars(file: string): Promise<Record<string, string>> {
  if (!existsSync(file)) return {};
  return parseEnvFile(await readFile(file, "utf-8"));
}

export const envRouter = new Hono();

// PUT/DELETE write to ~/.riku/envs/<app>/ENV — gate state-changing methods.
// GET /env/:app stays read-only and unauthenticated.
envRouter.on(["PUT", "DELETE"], "/env/*", requireMutatingAuth);

// ── GET /env/:app ── Read env vars as JSON ──
envRouter.get("/env/:app", async (c) => {
  const { app: appName } = c.req.param();
  const file = envFilePath(appName);

  try {
    const vars = Object.entries(await readVars(file)).map(([key, value]) => ({
      key,
      value,
    }));
    return c.json({ app: appName, vars });
  } catch (e) {
    return c.json(
      { error: "failed to read env file", detail: e instanceof Error ? e.message : String(e) },
      500,
    );
  }
});

// ── PUT /env/:app ── Set a single env var (body: { key, value }) ──
envRouter.put("/env/:app", async (c) => {
  const { app: appName } = c.req.param();
  const { key, value } = await c.req.json<{ key: string; value: string }>();

  if (!key || typeof key !== "string") {
    return c.json({ error: "missing or invalid key" }, 400);
  }

  const file = envFilePath(appName);

  try {
    await mkdir(join(ENVS_ROOT, appName), { recursive: true });
    const vars = await readVars(file);
    vars[key] = value ?? "";
    await writeFile(file, serializeEnvFile(vars), "utf-8");
    return c.json({ ok: true });
  } catch (e) {
    return c.json(
      { error: "failed to write env file", detail: e instanceof Error ? e.message : String(e) },
      500,
    );
  }
});

// ── DELETE /env/:app ── Delete a single env var (body: { key }) ──
envRouter.delete("/env/:app", async (c) => {
  const { app: appName } = c.req.param();
  const { key } = await c.req.json<{ key: string }>();

  if (!key || typeof key !== "string") {
    return c.json({ error: "missing or invalid key" }, 400);
  }

  const file = envFilePath(appName);
  if (!existsSync(file)) return c.json({ ok: true });

  try {
    const vars = await readVars(file);
    delete vars[key];
    await writeFile(file, serializeEnvFile(vars), "utf-8");
    return c.json({ ok: true });
  } catch (e) {
    return c.json(
      { error: "failed to delete env var", detail: e instanceof Error ? e.message : String(e) },
      500,
    );
  }
});
