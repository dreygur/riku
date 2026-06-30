// Client API — all calls go through the same-origin /api/riku proxy.
import type {
  RikuState,
  Release,
  DoctorCheck,
  AddonInstance,
  PluginsList,
} from "./types";

const base = "/api/riku";

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`${base}/${path}`, { cache: "no-store" });
  if (!res.ok) throw new Error(`${res.status} ${await res.text()}`);
  return res.json() as Promise<T>;
}

async function send(path: string, method: string, body?: unknown): Promise<void> {
  const res = await fetch(`${base}/${path}`, {
    method,
    headers: { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!res.ok) throw new Error((await res.text()) || `${res.status}`);
}

export const api = {
  state: () => get<RikuState>("state"),
  releases: (app: string) => get<Release[]>(`apps/${app}/releases`),
  env: (app: string) => get<Record<string, string>>(`apps/${app}/env`),
  doctor: () => get<DoctorCheck[]>("doctor"),
  addons: () => get<AddonInstance[]>("addons"),
  plugins: () => get<PluginsList>("plugins"),

  restart: (app: string) => send(`apps/${app}/restart`, "POST"),
  stop: (app: string) => send(`apps/${app}/stop`, "POST"),
  redeploy: (app: string) => send(`apps/${app}/redeploy`, "POST"),
  scale: (app: string, kinds: Record<string, number>) =>
    send(`apps/${app}/scale`, "POST", kinds),
  rollback: (app: string, to?: string) =>
    send(`apps/${app}/rollback`, "POST", to ? { to } : {}),
  setEnv: (app: string, set: Record<string, string>, unset: string[] = []) =>
    send(`apps/${app}/env`, "POST", { set, unset }),
  backup: (app: string) => send(`apps/${app}/backup`, "POST"),

  // addons (managed datastores)
  addonCreate: (plugin: string, instance: string) =>
    send("addons", "POST", { plugin, instance }),
  addonBind: (instance: string, app: string) => send(`addons/${instance}/bind`, "POST", { app }),
  addonUnbind: (instance: string, app: string) =>
    send(`addons/${instance}/unbind`, "POST", { app }),
  addonBackup: (instance: string) => send(`addons/${instance}/backup`, "POST"),
  addonDestroy: (instance: string) => send(`addons/${instance}`, "DELETE"),

  // SSE stream URL (consumed by EventSource)
  logsUrl: (app: string) => `${base}/apps/${app}/logs`,
};

export const fmtBytes = (b: number) => {
  if (!b) return "0";
  const u = ["B", "K", "M", "G"];
  let i = 0;
  let n = b;
  while (n >= 1024 && i < u.length - 1) {
    n /= 1024;
    i++;
  }
  return (n >= 100 || i === 0 ? Math.round(n) : n.toFixed(1)) + u[i];
};

export const fmtDur = (s: number) => {
  s = Math.max(0, s | 0);
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (d) return `${d}d${h}h`;
  if (h) return `${h}h${m}m`;
  return `${m}m`;
};
