/**
 * Same-origin proxy to the riku binary's embedded API.
 *
 * The browser talks only to this Next route; we forward to the binary and
 * attach the control token server-side, so the token never reaches the client.
 * Streaming responses (SSE: logs, metrics) pass straight through.
 */
import { type NextRequest } from "next/server";

const BASE = process.env.RIKU_API_URL ?? "http://127.0.0.1:8088";
const TOKEN = process.env.RIKU_DASHBOARD_TOKEN ?? "";

export const dynamic = "force-dynamic";

async function proxy(req: NextRequest, path: string[]): Promise<Response> {
  const search = new URL(req.url).search;
  const target = `${BASE}/api/${path.join("/")}${search}`;

  const headers: Record<string, string> = {};
  if (TOKEN) headers["authorization"] = `Bearer ${TOKEN}`;

  const init: RequestInit & { duplex?: "half" } = { method: req.method, headers };
  if (req.method !== "GET" && req.method !== "HEAD") {
    // Forward the original content-type: most calls are JSON, but uploads
    // (e.g. the restore endpoint's tar.gz) are multipart/form-data,
    // rewriting that to application/json would corrupt the multipart
    // boundary and the file. Stream the body through rather than buffering
    // it as text, so a large backup archive isn't held fully in memory here.
    headers["content-type"] = req.headers.get("content-type") ?? "application/json";
    init.body = req.body;
    init.duplex = "half";
  }

  let upstream: Response;
  try {
    upstream = await fetch(target, init);
  } catch {
    return new Response("supervisor unreachable", { status: 502 });
  }

  // Pass the body through verbatim: this keeps SSE streams live.
  return new Response(upstream.body, {
    status: upstream.status,
    headers: {
      "content-type": upstream.headers.get("content-type") ?? "application/json",
      "cache-control": "no-store",
    },
  });
}

type Ctx = { params: Promise<{ path: string[] }> };
const handler = async (req: NextRequest, ctx: Ctx) =>
  proxy(req, (await ctx.params).path);

export const GET = handler;
export const POST = handler;
export const PUT = handler;
export const DELETE = handler;
export const PATCH = handler;
