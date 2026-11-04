/**
 * Session cookie for the dashboard's single-shared-password login.
 *
 * Runs in the Node proxy and the route handlers of one process. It only uses
 * Web Crypto (`crypto.subtle`, `atob`/`btoa`), no `Buffer`, no Node `crypto`.
 *
 * The token is `${expiryMs}.${hmacSignature}`, signed with an HMAC key
 * derived from `SHA-256(RIKU_DASHBOARD_PASSWORD_HASH)`, the stored
 * (already-hashed) secret, never the plaintext password, so no separate
 * signing secret needs to be provisioned or rotated, and the plaintext
 * password never has to be read back out of anything at rest.
 *
 * The signed payload also carries the session epoch, kept next to the secret
 * in `process.env`. Logout bumps the epoch, which invalidates every token
 * issued before it. The epoch is seeded with the boot time rather than a
 * fixed value, so a restart cannot bring back a token revoked before it; a
 * restart signs everyone out instead.
 */

export const SESSION_COOKIE = "riku_session";
export const SESSION_TTL_MS = 12 * 60 * 60 * 1000;

const EPOCH_VAR = "RIKU_DASHBOARD_SESSION_EPOCH";

function sessionEpoch(): string {
  const current = process.env[EPOCH_VAR];
  if (current && Number.isFinite(Number(current))) return current;
  const seeded = String(Date.now());
  process.env[EPOCH_VAR] = seeded;
  return seeded;
}

/** Invalidates every session token issued so far. */
export function revokeAllSessions(): void {
  process.env[EPOCH_VAR] = String(Number(sessionEpoch()) + 1);
}

function signedBytes(expiry: string): Uint8Array<ArrayBuffer> {
  return new TextEncoder().encode(`${expiry}:${sessionEpoch()}`);
}

async function hmacKey(secret: string): Promise<CryptoKey> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(secret));
  return crypto.subtle.importKey("raw", digest, { name: "HMAC", hash: "SHA-256" }, false, [
    "sign",
    "verify",
  ]);
}

function bytesToBase64Url(bytes: Uint8Array): string {
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function base64UrlToBytes(s: string): Uint8Array<ArrayBuffer> {
  const b64 = s.replace(/-/g, "+").replace(/_/g, "/") + "=".repeat((4 - (s.length % 4)) % 4);
  const binary = atob(b64);
  const bytes = new Uint8Array(new ArrayBuffer(binary.length));
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

export async function createSessionToken(secret: string): Promise<string> {
  const payload = String(Date.now() + SESSION_TTL_MS);
  const key = await hmacKey(secret);
  const sig = await crypto.subtle.sign("HMAC", key, signedBytes(payload));
  return `${payload}.${bytesToBase64Url(new Uint8Array(sig))}`;
}

export async function verifySessionToken(
  token: string | undefined,
  secret: string,
): Promise<boolean> {
  if (!token) return false;
  const dot = token.indexOf(".");
  if (dot < 0) return false;

  const payload = token.slice(0, dot);
  const exp = Number(payload);
  if (!Number.isFinite(exp) || Date.now() > exp) return false;

  try {
    const key = await hmacKey(secret);
    const sig = base64UrlToBytes(token.slice(dot + 1));
    return await crypto.subtle.verify("HMAC", key, sig, signedBytes(payload));
  } catch {
    return false;
  }
}
