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

export interface NginxState {
  config_exists: boolean;
  enabled: boolean;
}

export interface AppState {
  app: string;
  deploy_lock: string; // "held" | "free"
  routing: Record<string, string>; // NGINX_SERVER_NAME, NGINX_HTTPS_ONLY, ...
  nginx: NginxState;
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

export interface PluginBundle {
  name: string;
  version: string;
  type: string;
  description: string | null;
}
export interface PluginsList {
  runtimes: string[];
  hooks: string[];
  bundles: PluginBundle[];
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

export const domainOf = (a: AppState) => a.routing?.NGINX_SERVER_NAME;
export const httpsOf = (a: AppState) => Boolean(a.routing?.NGINX_HTTPS_ONLY);
export const isBusy = (a: AppState) => (a.deploy_lock || "").toLowerCase() === "held";
