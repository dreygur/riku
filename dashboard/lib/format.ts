// Small formatting helpers shared by the dense, htop-style dashboard views.

/** Format bytes as a fixed-width-ish human string, e.g. "128.4 MB". */
export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const exp = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  const value = bytes / 1024 ** exp;
  return `${value.toFixed(exp === 0 ? 0 : 1)} ${units[exp]}`;
}

/** Format milliseconds of CPU time as "Hh Mm Ss" / "Ms" style duration. */
export function formatCpuTime(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}h${minutes.toString().padStart(2, "0")}m`;
  if (minutes > 0) return `${minutes}m${seconds.toString().padStart(2, "0")}s`;
  return `${seconds}s`;
}

/** Format an ISO date string as a short, fixed-width local date. */
export function formatDate(iso: string | null): string {
  if (!iso) return "--";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "--";
  return d.toISOString().slice(0, 10);
}

/** Days remaining until an ISO date, or null if no date. */
export function daysUntil(iso: string | null): number | null {
  if (!iso) return null;
  const target = new Date(iso).getTime();
  if (Number.isNaN(target)) return null;
  return Math.ceil((target - Date.now()) / 86_400_000);
}

/** Pull the port out of an nginx upstream target, e.g. "http://127.0.0.1:5000" -> "5000". */
export function parseUpstreamPort(upstream: string | null): string | null {
  if (!upstream) return null;
  const match = upstream.match(/:(\d+)\/?$/);
  return match ? match[1] : null;
}

/** Format an ISO timestamp as elapsed time since now, e.g. "3h12m ago". */
export function formatAgo(iso: string | null): string {
  if (!iso) return "--";
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "--";
  const totalSeconds = Math.max(0, Math.floor((Date.now() - then) / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}h${minutes.toString().padStart(2, "0")}m ago`;
  if (minutes > 0) return `${minutes}m${seconds.toString().padStart(2, "0")}s ago`;
  return `${seconds}s ago`;
}
