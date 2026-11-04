/**
 * Per-client backoff for `/api/login`, held in this process's memory.
 *
 * The fifth failure locks the client out for 30 s, and each failure after
 * that doubles the lock, up to 15 min. A successful login clears the
 * client's record, and so does 30 min without a failure. State is lost on
 * restart, which only ever loosens the limit, never locks the owner out for
 * good.
 *
 * The client key comes from `clientKey`, see there for what it trusts.
 */

const LOCK_AFTER = 5;
const BASE_LOCK_MS = 30_000;
const MAX_LOCK_MS = 15 * 60_000;
const FORGET_AFTER_MS = 2 * MAX_LOCK_MS;
const PRUNE_ABOVE = 1_000;

interface Attempts {
  failures: number;
  lastFailure: number;
  lockedUntil: number;
}

const records = new Map<string, Attempts>();

/**
 * The client address as seen by riku's router: the last `X-Forwarded-For`
 * entry. nginx appends `$remote_addr` and Caddy replaces the header with the
 * peer address, so the last entry is the one the client cannot forge. Next
 * fills the header with the socket address when nothing set it.
 *
 * This only holds while the dashboard is reachable through riku's proxy or
 * loopback alone (`next start -H 127.0.0.1`, as documented). A dashboard
 * bound to a public address lets clients pick their own key.
 */
export function clientKey(headers: Headers): string {
  const forwarded = headers.get("x-forwarded-for");
  if (!forwarded) return "unknown";
  const last = forwarded.split(",").pop()?.trim();
  return last || "unknown";
}

function isStale(record: Attempts, now: number): boolean {
  return now >= record.lockedUntil && now - record.lastFailure > FORGET_AFTER_MS;
}

function prune(now: number): void {
  if (records.size <= PRUNE_ABOVE) return;
  for (const [key, record] of records) {
    if (isStale(record, now)) records.delete(key);
  }
}

/** Milliseconds `key` must still wait before its next attempt, 0 when free. */
export function lockRemaining(key: string, now = Date.now()): number {
  const record = records.get(key);
  if (!record) return 0;
  return Math.max(0, record.lockedUntil - now);
}

/**
 * Counts an attempt against `key`. The login route calls this before checking
 * the password, so a burst of parallel guesses is counted before any of the
 * slow hash checks finish.
 */
export function recordFailure(key: string, now = Date.now()): void {
  prune(now);
  const previous = records.get(key);
  const failures = previous && !isStale(previous, now) ? previous.failures + 1 : 1;
  const lockedUntil =
    failures >= LOCK_AFTER
      ? now + Math.min(BASE_LOCK_MS * 2 ** (failures - LOCK_AFTER), MAX_LOCK_MS)
      : 0;
  records.set(key, { failures, lastFailure: now, lockedUntil });
}

export function recordSuccess(key: string): void {
  records.delete(key);
}
