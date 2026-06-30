import type { ProcStatus } from "./types";

export type DotKind = "alive" | "warn" | "dead" | "idle";

export function statusMeta(s: ProcStatus): { dot: DotKind; label: string } {
  switch (s) {
    case "running":
      return { dot: "alive", label: "running" };
    case "starting":
    case "restarting":
      return { dot: "warn", label: s };
    case "crashed":
      return { dot: "dead", label: "crashed" };
    case "oom_killed":
      return { dot: "dead", label: "oom-killed" };
    default:
      return { dot: "idle", label: s || "stopped" };
  }
}
