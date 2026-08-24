"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { ChevronDownIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { StatusDot } from "./status-dot";
import { LogSheet } from "./log-sheet";
import { confirmDialog } from "./confirm-dialog";
import { api, fmtBytes } from "@/lib/api";
import { usePendingRun } from "@/lib/use-pending-run";
import { statusMeta } from "@/lib/status";
import { domainOf, httpsOf, isBusy, type AppState, type Release } from "@/lib/types";

const MEM_CEIL = 256 * 1024 * 1024;

export function AppCard({ app, onChanged }: { app: AppState; onChanged: () => void }) {
  const [logsOpen, setLogsOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const [releases, setReleases] = useState<Release[]>([]);
  const [rollbackTo, setRollbackTo] = useState<string>("");
  const busy = isBusy(app);
  const domain = domainOf(app);
  const https = httpsOf(app);
  const webCount = app.workers.filter((w) => w.kind === "web").length || 1;
  const [scaleN, setScaleN] = useState(webCount);
  const { isPending, run: runPending } = usePendingRun();

  useEffect(() => {
    api.releases(app.app).then(setReleases).catch(() => {});
  }, [app.app]);

  function run(key: string, label: string, fn: () => Promise<void>) {
    runPending(key, `${label} ${app.app}`, async () => {
      await fn();
      setTimeout(onChanged, 600);
    });
  }

  const workers = [...app.workers].sort((a, b) =>
    (a.kind + a.ordinal).localeCompare(b.kind + b.ordinal),
  );

  return (
    <section className="border border-border bg-card">
      {/* header */}
      <div className="flex flex-wrap items-center gap-3 px-4 pt-4 pb-3">
        <button
          onClick={() => setCollapsed((c) => !c)}
          className="text-muted-foreground hover:text-foreground"
          aria-label={collapsed ? "expand" : "collapse"}
          aria-expanded={!collapsed}
        >
          <ChevronDownIcon
            className={`size-4 transition-transform ${collapsed ? "-rotate-90" : ""}`}
          />
        </button>
        <Link
          href={`/apps/${app.app}`}
          className="font-mono text-base font-bold hover:text-primary"
        >
          {app.app}
        </Link>
        {domain && (
          <span className="border border-info/30 px-2 py-0.5 font-mono text-[11px] text-info">
            {https ? "https://" : "http://"}
            {domain}
          </span>
        )}
        <span
          className={`border px-2 py-0.5 font-mono text-[11px] ${
            app.nginx.enabled
              ? "border-primary/30 text-primary"
              : "border-border text-muted-foreground"
          }`}
        >
          nginx {app.nginx.enabled ? "live" : app.nginx.config_exists ? "staged" : "off"}
        </span>
        {busy && (
          <span className="border border-warn/40 px-2 py-0.5 font-mono text-[11px] text-warn">
            deploying…
          </span>
        )}
        {collapsed && (
          <span className="font-mono text-[11px] text-muted-foreground">
            {workers.length} worker{workers.length === 1 ? "" : "s"}
          </span>
        )}
      </div>

      {!collapsed && (
        <>
          {/* workers */}
          <div className="border-t border-border">
            {workers.length === 0 ? (
              <div className="px-4 py-2.5 font-mono text-xs text-muted-foreground">
                no workers running
              </div>
            ) : (
              workers.map((w) => {
                const m = statusMeta(w.status);
                const pct = Math.min(100, ((w.memory_bytes || 0) / MEM_CEIL) * 100);
                // Only a worker that's actually down (not mid-transition) can be
                // usefully restarted or deleted on its own: a config file is
                // guaranteed to still exist for it (the API hides stopped rows
                // whose config was scaled away entirely), so both actions are
                // always safe to offer here.
                const terminal = !["running", "starting", "restarting"].includes(w.status);
                const restartKey = `restart-${w.process_id}`;
                const deleteKey = `delete-${w.process_id}`;
                return (
                  <div
                    key={w.process_id}
                    className="flex flex-col gap-1.5 border-b border-border/50 px-4 py-2.5 font-mono text-xs last:border-b-0 sm:grid sm:grid-cols-[16px_140px_1fr_1fr_60px_auto] sm:items-center sm:gap-3"
                  >
                    {/* sm:contents makes this wrapper transparent to the grid at
                        sm+ (its two children become grid items 1 and 2 directly),
                        while still acting as one combined flex row on mobile so
                        the dot doesn't sit alone on its own line. */}
                    <span className="flex items-center gap-2 sm:contents">
                      <StatusDot kind={m.dot} />
                      <span>
                        <b className="text-foreground">
                          {w.kind}.{w.ordinal}
                        </b>{" "}
                        <span className="text-muted-foreground">{m.label}</span>
                      </span>
                    </span>
                    <span className="grid grid-cols-[28px_1fr_52px] items-center gap-2">
                      <span className="text-[9px] tracking-wider text-muted-foreground uppercase">
                        mem
                      </span>
                      <span className="block h-[6px] w-full bg-border">
                        <span
                          className="block h-full bg-info"
                          style={{ width: `${pct}%` }}
                        />
                      </span>
                      <span className="text-right text-[11px] text-muted-foreground">
                        {fmtBytes(w.memory_bytes)}
                      </span>
                    </span>
                    <span className="flex items-center justify-between gap-2 sm:contents">
                      <span className="text-muted-foreground">
                        {w.pid ? `pid ${w.pid}` : "-"} · cpu{" "}
                        {Math.round((w.cpu_time_ms || 0) / 1000)}s
                      </span>
                      <span
                        className={`sm:text-right ${
                          w.restart_count > 0 ? "text-warn" : "text-muted-foreground"
                        }`}
                      >
                        ↻{w.restart_count}
                      </span>
                    </span>
                    <span className="flex items-center gap-1">
                      {terminal && (
                        <>
                          <Button
                            size="sm"
                            variant="secondary"
                            className="h-6 px-2 text-[10px]"
                            disabled={isPending(restartKey)}
                            onClick={() =>
                              run(restartKey, `Restarted ${w.kind}.${w.ordinal} of`, () =>
                                api.restartWorker(app.app, w.kind, w.ordinal),
                              )
                            }
                          >
                            {isPending(restartKey) ? "restarting…" : "restart"}
                          </Button>
                          <Button
                            size="sm"
                            variant="destructive"
                            className="h-6 px-2 text-[10px]"
                            disabled={isPending(deleteKey)}
                            onClick={async () => {
                              const ok = await confirmDialog(
                                `Delete ${w.kind}.${w.ordinal}? This removes just this instance.`,
                              );
                              if (ok)
                                run(deleteKey, `Deleted ${w.kind}.${w.ordinal} of`, () =>
                                  api.deleteWorker(app.app, w.kind, w.ordinal),
                                );
                            }}
                          >
                            {isPending(deleteKey) ? "…" : "×"}
                          </Button>
                        </>
                      )}
                    </span>
                  </div>
                );
              })
            )}
          </div>

          {/* actions */}
          <div className="flex flex-wrap items-center gap-2 border-t border-border bg-black/10 px-4 py-3">
            <Button size="sm" variant="secondary" onClick={() => setLogsOpen(true)}>
              logs
            </Button>
            <Button
              size="sm"
              variant="secondary"
              disabled={busy || isPending("restart")}
              onClick={() => run("restart", "Restarted", () => api.restart(app.app))}
            >
              {isPending("restart") ? "restarting…" : "restart"}
            </Button>
            <Button
              size="sm"
              variant="accent"
              disabled={busy || isPending("redeploy")}
              onClick={() => run("redeploy", "Redeployed", () => api.redeploy(app.app))}
            >
              {isPending("redeploy") ? "redeploying…" : "redeploy"}
            </Button>

            <span className="ml-1 inline-flex items-center gap-1.5">
              <span className="text-[10px] tracking-wider text-muted-foreground uppercase">
                scale web
              </span>
              <Input
                type="number"
                min={0}
                max={32}
                value={scaleN}
                onChange={(e) => setScaleN(Number(e.target.value))}
                className="h-7 w-16 font-mono"
              />
              <Button
                size="sm"
                variant="secondary"
                disabled={isPending("scale")}
                onClick={() => run("scale", "Scaled", () => api.scale(app.app, { web: scaleN }))}
              >
                {isPending("scale") ? "scaling…" : "set"}
              </Button>
            </span>

            <span className="flex-1" />

            <Select value={rollbackTo} onValueChange={setRollbackTo}>
              <SelectTrigger size="sm" className="w-44 font-mono text-[11px]">
                <SelectValue placeholder="roll back to…" />
              </SelectTrigger>
              {/* item-aligned (Radix's default) positions the popover relative to
                  the selected item's own layout, which breaks inside this
                  flex-wrap action row (renders overlapping neighboring buttons).
                  popper mode anchors it as a normal floating dropdown instead. */}
              <SelectContent position="popper">
                {releases
                  .slice()
                  .reverse()
                  .slice(0, 20)
                  .map((r) => (
                    <SelectItem key={r.sha} value={r.sha} className="font-mono text-[11px]">
                      {r.sha.slice(0, 8)} · {new Date(r.ts * 1000).toLocaleString()}
                    </SelectItem>
                  ))}
              </SelectContent>
            </Select>
            <Button
              size="sm"
              variant="secondary"
              disabled={isPending("rollback")}
              onClick={() =>
                run("rollback", "Rolled back", () => api.rollback(app.app, rollbackTo || undefined))
              }
            >
              {isPending("rollback") ? "rolling back…" : "go"}
            </Button>
            <Button
              size="sm"
              variant="destructive"
              disabled={busy || isPending("stop")}
              onClick={async () => {
                const ok = await confirmDialog(`Stop ${app.app}? Its workers will be shut down.`);
                if (ok) run("stop", "Stopped", () => api.stop(app.app));
              }}
            >
              {isPending("stop") ? "stopping…" : "stop"}
            </Button>
          </div>
        </>
      )}

      <LogSheet app={app.app} open={logsOpen} onOpenChange={setLogsOpen} />
    </section>
  );
}
