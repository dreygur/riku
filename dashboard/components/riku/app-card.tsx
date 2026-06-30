"use client";

import { useEffect, useState } from "react";
import { toast } from "sonner";
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
import { api, fmtBytes } from "@/lib/api";
import { statusMeta } from "@/lib/status";
import { domainOf, httpsOf, isBusy, type AppState, type Release } from "@/lib/types";

const MEM_CEIL = 256 * 1024 * 1024;

export function AppCard({ app, onChanged }: { app: AppState; onChanged: () => void }) {
  const [logsOpen, setLogsOpen] = useState(false);
  const [releases, setReleases] = useState<Release[]>([]);
  const [rollbackTo, setRollbackTo] = useState<string>("");
  const busy = isBusy(app);
  const domain = domainOf(app);
  const https = httpsOf(app);
  const webCount = app.workers.filter((w) => w.kind === "web").length || 1;
  const [scaleN, setScaleN] = useState(webCount);

  useEffect(() => {
    api.releases(app.app).then(setReleases).catch(() => {});
  }, [app.app]);

  async function run(label: string, fn: () => Promise<void>) {
    try {
      await fn();
      toast.success(`${label} ${app.app}`);
      setTimeout(onChanged, 600);
    } catch (e) {
      toast.error(`${label} failed: ${(e as Error).message}`);
    }
  }

  const workers = [...app.workers].sort((a, b) =>
    (a.kind + a.ordinal).localeCompare(b.kind + b.ordinal),
  );

  return (
    <section className="border border-border bg-card">
      {/* header */}
      <div className="flex flex-wrap items-center gap-3 px-4 pt-4 pb-3">
        <h2 className="font-mono text-base font-bold">{app.app}</h2>
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
          <span className="border border-[var(--warn)]/40 px-2 py-0.5 font-mono text-[11px] text-[var(--warn)]">
            deploying…
          </span>
        )}
      </div>

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
            return (
              <div
                key={w.process_id}
                className="grid grid-cols-[16px_140px_1fr_1fr_60px] items-center gap-3 border-b border-border/50 px-4 py-2.5 font-mono text-xs last:border-b-0"
              >
                <StatusDot kind={m.dot} />
                <span>
                  <b className="text-foreground">
                    {w.kind}.{w.ordinal}
                  </b>{" "}
                  <span className="text-muted-foreground">{m.label}</span>
                </span>
                <span className="grid grid-cols-[28px_1fr_52px] items-center gap-2">
                  <span className="text-[9px] tracking-wider text-muted-foreground uppercase">
                    mem
                  </span>
                  <span className="block h-[6px] w-full bg-[#242a33]">
                    <span className="block h-full bg-[#5b9dd9]" style={{ width: `${pct}%` }} />
                  </span>
                  <span className="text-right text-[11px] text-muted-foreground">
                    {fmtBytes(w.memory_bytes)}
                  </span>
                </span>
                <span className="text-muted-foreground">
                  {w.pid ? `pid ${w.pid}` : "—"} · cpu {Math.round((w.cpu_time_ms || 0) / 1000)}s
                </span>
                <span
                  className={`text-right ${
                    w.restart_count > 0 ? "text-[var(--warn)]" : "text-muted-foreground"
                  }`}
                >
                  ↻{w.restart_count}
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
          disabled={busy}
          onClick={() => run("Restarted", () => api.restart(app.app))}
        >
          restart
        </Button>
        <Button
          size="sm"
          variant="secondary"
          disabled={busy}
          className="border-primary/30"
          onClick={() => run("Redeployed", () => api.redeploy(app.app))}
        >
          redeploy
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
            className="h-8 w-16 font-mono"
          />
          <Button
            size="sm"
            variant="secondary"
            onClick={() => run("Scaled", () => api.scale(app.app, { web: scaleN }))}
          >
            set
          </Button>
        </span>

        <span className="flex-1" />

        <Select value={rollbackTo} onValueChange={setRollbackTo}>
          <SelectTrigger size="sm" className="w-44 font-mono text-[11px]">
            <SelectValue placeholder="roll back to…" />
          </SelectTrigger>
          <SelectContent>
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
          onClick={() => run("Rolled back", () => api.rollback(app.app, rollbackTo || undefined))}
        >
          go
        </Button>
        <Button
          size="sm"
          variant="secondary"
          disabled={busy}
          className="hover:border-destructive/50 hover:text-destructive"
          onClick={() => {
            if (confirm(`Stop ${app.app}? Its workers will be shut down.`))
              run("Stopped", () => api.stop(app.app));
          }}
        >
          stop
        </Button>
      </div>

      <LogSheet app={app.app} open={logsOpen} onOpenChange={setLogsOpen} />
    </section>
  );
}
