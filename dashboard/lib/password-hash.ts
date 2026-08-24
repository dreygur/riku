/**
 * Password hashing for `RIKU_DASHBOARD_PASSWORD_HASH`.
 *
 * Node-only (uses `crypto.scrypt`): do not import this from `middleware.ts`,
 * which runs on the Edge runtime. The stored value is `${saltHex}:${hashHex}`;
 * the plaintext password itself is never persisted, only compared transiently
 * during a login POST.
 */
import { randomBytes, scrypt, timingSafeEqual } from "node:crypto";
import { promisify } from "node:util";

const scryptAsync = promisify(scrypt);
const KEYLEN = 64;

export async function hashPassword(password: string): Promise<string> {
  const salt = randomBytes(16);
  const hash = (await scryptAsync(password, salt, KEYLEN)) as Buffer;
  return `${salt.toString("hex")}:${hash.toString("hex")}`;
}

export async function verifyPassword(attempt: string, stored: string): Promise<boolean> {
  const sep = stored.indexOf(":");
  if (sep < 0) return false;

  const salt = Buffer.from(stored.slice(0, sep), "hex");
  const expected = Buffer.from(stored.slice(sep + 1), "hex");
  const actual = (await scryptAsync(attempt, salt, KEYLEN)) as Buffer;

  return actual.length === expected.length && timingSafeEqual(actual, expected);
}
