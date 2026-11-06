/**
 * Post-login redirect target taken from the `from` query parameter.
 *
 * The parameter is attacker controlled (anyone can send a login link), so
 * only a same-origin path is kept. Absolute URLs, protocol-relative `//host`
 * and backslash forms that browsers treat as `//host` all fall back to `/`.
 *
 * The normalised result is checked again: dot segments such as
 * `/.//evil.example` resolve to the pathname `//evil.example`, which is
 * same-origin for the probe but protocol-relative once handed to the browser.
 */
const PROBE_ORIGIN = "http://riku.invalid";

function isLocalPath(path: string): boolean {
  return path.startsWith("/") && !path.startsWith("//") && !path.startsWith("/\\");
}

export function safeRedirectPath(from: string | null): string {
  if (!from || !from.startsWith("/")) return "/";
  let url: URL;
  try {
    url = new URL(from, PROBE_ORIGIN);
  } catch {
    return "/";
  }
  if (url.origin !== PROBE_ORIGIN) return "/";
  const target = url.pathname + url.search + url.hash;
  return isLocalPath(target) ? target : "/";
}
