// Types mirroring the riku binary's /api JSON. Status/health are snake_case.

export type ProcStatus =
  | "running"
  | "starting"
  | "stopped"
  | "crashed"
  | "restarting"
  | "oom_killed";

export interface Worker {
  process_id: string;
  kind: string;
  ordinal: number;
  pid: number | null;
  status: ProcStatus;
  restart_count: number;
  memory_bytes: number;
  cpu_time_ms: number;
}

// Mirrors `RouteState` in crates/riku-cli/src/apps/dump_state/route.rs.
export type RouteState =
  | { router: "nginx"; config_exists: boolean; enabled: boolean }
  | { router: "caddy"; site: string | null }
  | { router: "plugin"; name: string }
  | { router: "unknown"; error: string };

export interface AppState {
  app: string;
  deploy_lock: string; // "held" | "free"
  routing: Record<string, string>; // RIKU_DOMAINS, RIKU_HTTPS_ONLY, ...
  route: RouteState;
  workers: Worker[];
}

export interface RikuState {
  generated_at: number;
  riku_version: string;
  supervisor_uptime_seconds: number;
  apps: AppState[];
}

export interface Release {
  ts: number;
  sha: string;
}

export type DoctorStatus = "ok" | "warn" | "fail";
export interface DoctorCheck {
  name: string;
  status: DoctorStatus;
  detail: string;
}

export interface AddonInstance {
  plugin: string;
  instance: string;
  bindings: Record<string, string[]>;
}

export interface PluginUi {
  nav_label: string;
}
export interface PluginBundle {
  name: string;
  version: string;
  type: string;
  description: string | null;
  author: string | null;
  ui: PluginUi | null;
}
export interface PluginsList {
  runtimes: string[];
  hooks: string[];
  bundles: PluginBundle[];
}

export interface PanelField {
  label: string;
  value: string;
}
export interface PanelSection {
  title: string;
  fields: PanelField[];
}
export interface UiPanel {
  sections: PanelSection[];
}

export interface MarketplaceSource {
  name: string;
  url: string;
}
export interface MarketplaceHit {
  marketplace: string;
  name: string;
  source: string;
  description: string | null;
}
export interface TrustKey {
  name: string;
  pubkey: string;
}

export const domainOf = (a: AppState) => a.routing?.RIKU_DOMAINS;
// Same on values as `riku_util::flag::is_on`; anything else, "false" included, is off.
const ON_VALUES = ["1", "true", "yes", "on"];
export const httpsOf = (a: AppState) =>
  ON_VALUES.includes((a.routing?.RIKU_HTTPS_ONLY ?? "").trim().toLowerCase());
// The app card badge: router name plus whether its config is serving.
export function routeBadge(route: RouteState): { label: string; live: boolean } {
  switch (route.router) {
    case "nginx":
      return {
        label: `nginx ${route.enabled ? "live" : route.config_exists ? "staged" : "off"}`,
        live: route.enabled,
      };
    case "caddy":
      return { label: `caddy ${route.site ? "live" : "off"}`, live: route.site !== null };
    case "plugin":
      return { label: route.name, live: true };
    case "unknown":
      return { label: "router unreadable", live: false };
  }
}
export const isBusy = (a: AppState) => (a.deploy_lock || "").toLowerCase() === "held";
