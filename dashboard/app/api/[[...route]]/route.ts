import { Hono } from "hono";
import { handle } from "hono/vercel";
import { controlRouter } from "@/server/routers/control";
import { envRouter } from "@/server/routers/env";
import { supervisorRouter } from "@/server/routers/supervisor";
import { allowedOrigin } from "@/server/security";

export const runtime = "nodejs";

// ── Security model & operator configuration ─────────────────────────────────
//
// This proxy deputizes the Rust control token (~/.riku/control.token) onto
// upstream /control/* calls, so the browser-facing side MUST defend against
// confused-deputy / CSRF abuse. Mutating routes (control router + env
// PUT/DELETE) are guarded in server/security.ts; CORS is locked here.
//
// Operator environment variables:
//   RIKU_DASHBOARD_TOKEN   Shared secret required (header x-riku-dashboard-token)
//                          on every mutating request. If UNSET, mutating routes
//                          fail closed with 503 — never default to open.
//   RIKU_DASHBOARD_ORIGIN  The single allowed browser origin for CORS + CSRF.
//                          Default http://127.0.0.1:3000. Must match the same
//                          env var on the Rust side (src/supervisor/health/mod.rs).
//
// Deployment: bind the Next.js server to loopback so the dashboard is not
// reachable from other hosts:
//   RIKU_DASHBOARD_TOKEN=<secret> next start -H 127.0.0.1

const app = new Hono().basePath("/api");

// ── CORS ── echo only the single allowed origin; never the `*` wildcard ──
app.use("*", async (c, next) => {
  await next();
  c.header("Access-Control-Allow-Origin", allowedOrigin());
  c.header("Vary", "Origin");
  c.header("Access-Control-Allow-Methods", "GET,POST,PUT,DELETE,OPTIONS");
  c.header("Access-Control-Allow-Headers", "x-riku-dashboard-token, Content-Type");
});

app.options("*", (c) => {
  c.header("Access-Control-Allow-Origin", allowedOrigin());
  c.header("Vary", "Origin");
  c.header("Access-Control-Allow-Methods", "GET,POST,PUT,DELETE,OPTIONS");
  c.header("Access-Control-Allow-Headers", "x-riku-dashboard-token, Content-Type");
  return c.body(null, 204);
});

app.route("/", supervisorRouter);
app.route("/", envRouter);
app.route("/", controlRouter);

export const GET = handle(app);
export const POST = handle(app);
export const PUT = handle(app);
export const DELETE = handle(app);
