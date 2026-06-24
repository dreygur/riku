import type { ProcessStatus } from "@/lib/types";

const STATUS_LABEL: Record<ProcessStatus, string> = {
  running: "RUNNING",
  starting: "STARTING",
  restarting: "RESTARTING",
  stopped: "STOPPED",
  crashed: "CRASHED",
  oom_killed: "OOM KILLED",
};

const STATUS_STYLE: Record<ProcessStatus, string> = {
  running: "text-accent-green",
  starting: "text-accent-amber",
  restarting: "text-accent-amber",
  stopped: "text-foreground-dim",
  crashed: "text-accent-red",
  oom_killed: "text-accent-red",
};

export function StatusTag({ status }: { status: ProcessStatus }) {
  return (
    <span className={`text-xs font-bold ${STATUS_STYLE[status]}`}>
      [{STATUS_LABEL[status]}]
    </span>
  );
}
