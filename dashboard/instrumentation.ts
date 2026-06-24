// ── Startup preflight ────────────────────────────────────────────────────────
//
// Next.js calls `register()` once when the server process boots. We use it to
// fail fast on an unsafe configuration instead of silently serving in a state
// where the operator believes they are protected but are not.
//
// The dashboard's API proxy holds the Rust control token and can read app env
// secrets. Mutating + env routes already fail closed (503/401) when
// `RIKU_DASHBOARD_TOKEN` is unset, so a token-less dashboard is "safe but
// useless". The danger is an operator running it token-less AND reachable off
// the loopback interface (`next start -H 0.0.0.0`), exposing the read surface
// to the LAN. This preflight refuses to start in that case, and also refuses
// any token-less production start, so misconfiguration surfaces at boot rather
// than as a quiet exposure later.

export async function register(): Promise<void> {
  // Only the Node.js server runtime touches the filesystem / control token;
  // the edge runtime has nothing to guard here.
  if (process.env.NEXT_RUNTIME !== "nodejs") return;

  const token = (process.env.RIKU_DASHBOARD_TOKEN ?? "").trim();
  if (token) return; // configured correctly — nothing to enforce

  // Best-effort bind-host detection. `next start -H <host>` is surfaced to the
  // app as HOSTNAME (and some setups use HOST); absent/loopback values are
  // treated as safe.
  const host = (process.env.HOSTNAME ?? process.env.HOST ?? "").trim().toLowerCase();
  const isLoopback =
    host === "" ||
    host === "127.0.0.1" ||
    host === "localhost" ||
    host === "::1" ||
    host === "[::1]";

  const isProduction = process.env.NODE_ENV === "production";

  if (!isLoopback || isProduction) {
    const reason = !isLoopback
      ? `the server is bound to a non-loopback host (${host})`
      : "the server is running in production mode";
    console.error(
      `[riku-dashboard] FATAL: RIKU_DASHBOARD_TOKEN is not set and ${reason}. ` +
        "The dashboard proxies the Rust control token and can read app env " +
        "secrets; refusing to start to avoid an unauthenticated exposure. " +
        "Set RIKU_DASHBOARD_TOKEN=<secret> and bind to loopback, e.g. " +
        "`RIKU_DASHBOARD_TOKEN=<secret> next start -H 127.0.0.1`.",
    );
    process.exit(1);
  }

  // Loopback + non-production with no token: allowed for local development,
  // but warn so the missing token is never a surprise.
  console.warn(
    "[riku-dashboard] WARNING: RIKU_DASHBOARD_TOKEN is not set. Mutating and " +
      "env routes will fail closed (503). Set it before exposing the dashboard.",
  );
}
