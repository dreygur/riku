import { Hono } from "hono";
import { handle } from "hono/vercel";
import { envRouter } from "@/server/routers/env";
import { supervisorRouter } from "@/server/routers/supervisor";

export const runtime = "nodejs";

const app = new Hono().basePath("/api");

// ── CORS ──
app.use("*", async (c, next) => {
  await next();
  c.header("Access-Control-Allow-Origin", "*");
  c.header("Access-Control-Allow-Methods", "GET,POST,PUT,DELETE,OPTIONS");
  c.header("Access-Control-Allow-Headers", "Content-Type");
});

app.options("*", (c) => new Response(null, { status: 204 }));

app.route("/", supervisorRouter);
app.route("/", envRouter);

export const GET = handle(app);
export const POST = handle(app);
export const PUT = handle(app);
export const DELETE = handle(app);
