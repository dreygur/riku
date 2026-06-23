import type { Context, Next } from "hono";
import { timingSafeEqual } from "node:crypto";

// ── Dashboard security model ────────────────────────────────────────────────
//
// The Next.js API proxy (/api/*) deputizes the Rust control token: it reads
// ~/.riku/control.token server-side and attaches `Authorization: Bearer` to
// every upstream /control/* call. Because the browser-facing side previously
// had no authentication and an `Access-Control-Allow-Origin: *` policy, ANY
// web page the operator visited could issue a cross-site state-changing
// request and have the dashboard relay the token (a confused-deputy / CSRF
// hole). This module re-establishes a trust boundary on mutating routes:
//
//   1. Operator token — a shared secret in `RIKU_DASHBOARD_TOKEN` must be
//      presented in the `x-riku-dashboard-token` header. Compared in constant
//      time. If the env var is unset we FAIL CLOSED (503), never open.
//   2. Same-origin / CSRF — the request `Origin` must match
//      `RIKU_DASHBOARD_ORIGIN` (default http://127.0.0.1:3000, kept in sync
//      with the Rust side in src/supervisor/health/mod.rs). Same-origin
//      navigations that omit Origin are accepted only when
//      `Sec-Fetch-Site: same-origin`; any cross-site value is rejected (403).
//
// Read-only GETs (health/metrics/network/logs) intentionally stay
// unauthenticated — they are gated separately by CORS.

const DEFAULT_ORIGIN = "http://127.0.0.1:3000";

/** The single allowed browser origin. Shared by CORS and CSRF checks. */
export function allowedOrigin(): string {
  const configured = (process.env.RIKU_DASHBOARD_ORIGIN ?? "").trim();
  return configured || DEFAULT_ORIGIN;
}

/** Constant-time string compare that guards length before comparing bytes. */
function constantTimeEquals(a: string, b: string): boolean {
  const bufA = Buffer.from(a, "utf-8");
  const bufB = Buffer.from(b, "utf-8");
  if (bufA.length !== bufB.length) return false;
  return timingSafeEqual(bufA, bufB);
}

/** Validate the operator token. Returns null on success or an error tuple. */
function checkToken(c: Context): { status: 401 | 503; error: string } | null {
  const expected = (process.env.RIKU_DASHBOARD_TOKEN ?? "").trim();
  if (!expected) {
    // Fail closed: refuse mutating actions until the operator configures a
    // token rather than silently allowing unauthenticated state changes.
    return { status: 503, error: "dashboard token not configured (set RIKU_DASHBOARD_TOKEN)" };
  }

  const presented = c.req.header("x-riku-dashboard-token") ?? "";
  if (!constantTimeEquals(presented, expected)) {
    return { status: 401, error: "invalid or missing dashboard token" };
  }
  return null;
}

/** True when the request is same-origin (matching Origin, or Origin-less with
 * `Sec-Fetch-Site: same-origin`). Used by both the mutating guard and the
 * token-delivery endpoint so a cross-site page can never read the token. */
export function isSameOrigin(c: Context): boolean {
  return checkOrigin(c) === null;
}

/** The operator token, or null when unconfigured. Never logged. */
export function dashboardToken(): string | null {
  const token = (process.env.RIKU_DASHBOARD_TOKEN ?? "").trim();
  return token || null;
}

/** Validate same-origin / CSRF posture. Returns null on success or an error. */
function checkOrigin(c: Context): { status: 403; error: string } | null {
  const allowed = allowedOrigin();
  const origin = c.req.header("origin");

  if (origin) {
    if (origin !== allowed) {
      return { status: 403, error: "cross-site origin rejected" };
    }
    return null;
  }

  // No Origin header: trust only an explicit same-origin fetch metadata signal.
  // Browsers send Sec-Fetch-Site on all modern requests; cross-site, same-site
  // and none are all unacceptable for a state-changing request without Origin.
  const fetchSite = c.req.header("sec-fetch-site");
  if (fetchSite === "same-origin") return null;

  return { status: 403, error: "missing Origin and non-same-origin request rejected" };
}

/**
 * Hono middleware that enforces the operator token + CSRF/same-origin checks.
 * Mount on every mutating route group (control router + mutating env/supervisor
 * handlers). Order: origin check first (cheap, no secret), then token.
 */
export async function requireMutatingAuth(c: Context, next: Next): Promise<Response | void> {
  const originErr = checkOrigin(c);
  if (originErr) return c.json({ ok: false, error: originErr.error }, originErr.status);

  const tokenErr = checkToken(c);
  if (tokenErr) return c.json({ ok: false, error: tokenErr.error }, tokenErr.status);

  await next();
}
