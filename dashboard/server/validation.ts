// ── App-name validation (trust boundary) ────────────────────────────────────
//
// Mirrors the Rust validator (src/util/validation.rs:validate_app_name) so the
// dashboard and the supervisor agree on what a legal app name is. Every route
// that turns an app name into a filesystem path (env, logs) MUST validate here
// before any path/fs operation — Node's `path.join` does NOT neutralize `..`,
// so an unvalidated name is a path-traversal vector.
//
// Strict-reject (not sanitize): the supervisor sanitizes by stripping bad
// characters, but at the HTTP boundary it is clearer and safer to reject a
// malformed name outright than to silently operate on a rewritten one.

/** Allowed app-name characters: alphanumeric, dot, underscore, hyphen. */
const APP_NAME_RE = /^[A-Za-z0-9._-]+$/;

/**
 * Return the app name if valid, or `null` if it must be rejected.
 *
 * Rejects: empty, characters outside `[A-Za-z0-9._-]`, any `..` sequence
 * (path traversal), and dot-only names (`.`, `..`, `...`).
 */
export function validateAppName(app: string | null | undefined): string | null {
  if (!app) return null;
  if (!APP_NAME_RE.test(app)) return null;
  if (app.includes("..")) return null;
  // Reject names that are nothing but dots (".", "..", "...") — they resolve
  // to the current/parent directory rather than a real app.
  if (app.replace(/\./g, "").length === 0) return null;
  return app;
}
