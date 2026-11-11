// @ts-check
// Escaping and the small text formatters every view shares. No DOM access.

const FLAG_ON = ["1", "true", "yes", "on"];

/**
 * Escapes text for HTML. Every server value that reaches innerHTML goes
 * through this, text and attributes alike.
 * @param {unknown} s
 * @returns {string}
 */
export const esc = (s) => String(s == null ? "" : s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

/** @param {string} s */
export const cap = (s) => (s ? s[0].toUpperCase() + s.slice(1) : s);

/**
 * Coerces anything to a number, with 0 for whatever does not parse.
 * @param {unknown} n
 * @returns {number}
 */
export const num = (n) => Number(n) || 0;

/**
 * Reads an env-style flag ("1", "true", "yes", "on").
 * @param {unknown} v
 */
export const isOn = (v) => FLAG_ON.includes(String(v == null ? "" : v).trim().toLowerCase());

/**
 * @param {number} n
 * @param {number} [l] width, 2 when omitted
 */
export const pad = (n, l) => String(n).padStart(l || 2, "0");

/** @param {unknown} b byte count */
export const fmtBytes = (b) => {
  let bytes = num(b);
  if (!bytes) { return "0"; }
  const u = ["B", "K", "M", "G"]; let i = 0; let n = bytes;
  while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
  return (n >= 100 || i === 0 ? Math.round(n) : n.toFixed(1)) + u[i];
};

/** @param {number} s seconds */
export const fmtDur = (s) => {
  s = Math.max(0, s | 0);
  const d = Math.floor(s / 86400), h = Math.floor((s % 86400) / 3600), m = Math.floor((s % 3600) / 60);
  if (d) { return d + "d " + h + "h"; }
  if (h) { return h + "h " + m + "m"; }
  return m + "m";
};

/**
 * The uptime part of the supervisor pill: the uptime, the server's reason
 * when it could not read one (a root run), or "uptime unknown".
 * @param {{ supervisor_uptime_seconds?: number | null, supervisor_uptime_unavailable?: string }} d
 */
export const uptimeText = (d) => {
  if (d.supervisor_uptime_seconds != null) { return "up " + fmtDur(d.supervisor_uptime_seconds); }
  return d.supervisor_uptime_unavailable || "uptime unknown";
};

/** @param {unknown} ms CPU time in milliseconds */
export const fmtCpu = (ms) => {
  let s = Math.round(num(ms) / 1000);
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60); s %= 60;
  return h ? h + "h " + pad(m) + "m" : m + "m " + pad(s) + "s";
};

/** @param {number} sec seconds since the event */
export const fmtAgo = (sec) => {
  const s = Math.max(0, Math.round(sec));
  if (s < 60) { return s <= 2 ? "just now" : s + "s ago"; }
  if (s < 3600) { return Math.floor(s / 60) + "m ago"; }
  if (s < 86400) { return Math.floor(s / 3600) + "h ago"; }
  return Math.floor(s / 86400) + "d ago";
};

/** @param {number} ts unix seconds */
export const fmtWhen = (ts) => {
  try { return new Date(ts * 1000).toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }); }
  catch (_) { return new Date(ts * 1000).toLocaleString(); }
};

/** @param {Date} d */
export const fmtClock = (d) => `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`;
