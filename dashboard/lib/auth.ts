/**
 * Session cookie for the dashboard's single-shared-password login.
 *
 * Runs in both the Edge middleware and Node route handlers, so this only
 * uses Web Crypto (`crypto.subtle`, `atob`/`btoa`), no `Buffer`, no Node
 * `crypto`: to stay portable across both runtimes.
 *
 * The token is `${expiryMs}.${hmacSignature}`, signed with an HMAC key
 * derived from `SHA-256(RIKU_DASHBOARD_PASSWORD_HASH)`, the stored
 * (already-hashed) secret, never the plaintext password, so no separate
 * signing secret needs to be provisioned or rotated, and the plaintext
 * password never has to be read back out of anything at rest.
 */

export const SESSION_COOKIE = "riku_session";
export const SESSION_TTL_MS = 7 * 24 * 60 * 60 * 1000;

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
  const sig = await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(payload));
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
    return await crypto.subtle.verify("HMAC", key, sig, new TextEncoder().encode(payload));
  } catch {
    return false;
  }
}
