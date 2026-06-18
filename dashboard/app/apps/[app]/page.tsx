"use client";

import * as React from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { StatusTag } from "@/components/StatusTag";
import { AppActionBar } from "@/components/AppActionBar";
import { NetworkPanel } from "@/components/NetworkPanel";
import { EnvEditor } from "@/components/EnvEditor";
import { TerminalStream } from "@/components/TerminalStream";
import { api } from "@/lib/api";
import { mapBackendToWorkers, type WorkerInfo } from "@/lib/types";
import { useAppActions } from "@/lib/useAppActions";
import { formatAgo, formatBytes, formatCpuTime } from "@/lib/format";

const POLL_INTERVAL_MS = 5_000;

export default function AppDetailPage() {
  const params = useParams<{ app: string }>();
  const app = decodeURIComponent(params.app);

  const [processes, setProcesses] = React.useState<WorkerInfo[]>([]);
  const [error, setError] = React.useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = React.useState<Date | null>(null);

  const actions = useAppActions(app);

  const fetchMetrics = React.useCallback(async () => {
    try {
      const stats = await api.metrics.getApp(app);
      setProcesses(mapBackendToWorkers([stats]));
      setLastUpdated(new Date());
      setError(null);
    } catch (e) {
      setProcesses([]);
      setError(e instanceof Error ? e.message : "connection refused");
    }
  }, [app]);

  React.useEffect(() => {
    fetchMetrics();
    const id = setInterval(fetchMetrics, POLL_INTERVAL_MS);
    return () => clearInterval(id);
  }, [fetchMetrics]);

  return (
    <div className="min-h-screen bg-background-dark text-foreground-dark font-mono flex flex-col">
      <header className="flex items-center justify-between border-b border-line px-4 py-3 shrink-0">
        <div className="flex items-center gap-3">
          <Link
            href="/"
            className="font-display text-xs font-bold tracking-wide text-foreground-muted hover:text-accent-amber"
          >
            ← ~/.riku
          </Link>
          <h1 className="font-display text-sm font-bold tracking-[0.15em]">
            ~/.riku/apps/{app}
          </h1>
        </div>
      </header>

      <div className="border-b border-line px-3 py-2">
        <AppActionBar
          app={app}
          pending={actions.pending}
          message={actions.message}
          isError={actions.isError}
          runAction={actions.runAction}
        />
      </div>

      <div className="border-b border-line">
        <div className="flex items-center justify-between px-3 py-2">
          <span className="font-display text-xs font-bold tracking-wide text-foreground-muted">
            process detail
          </span>
          <span className="text-xs text-foreground-muted tabular">
            {error ? (
              <span className="text-accent-red">[{error}]</span>
            ) : lastUpdated ? (
              `updated ${lastUpdated.toLocaleTimeString()}`
            ) : (
              "loading..."
            )}
          </span>
        </div>

        <div className="overflow-x-auto">
          <Table className="rounded-none text-sm">
            <TableHeader>
              <TableRow className="rounded-none border-b border-line/30">
                <TableHead className="rounded-none">PROC</TableHead>
                <TableHead className="rounded-none text-right">PID</TableHead>
                <TableHead className="rounded-none">STATUS</TableHead>
                <TableHead className="rounded-none">HEALTH</TableHead>
                <TableHead className="rounded-none">STARTED</TableHead>
                <TableHead className="rounded-none">LAST RESTART</TableHead>
                <TableHead className="rounded-none text-right">RESTARTS</TableHead>
                <TableHead className="rounded-none text-right">RSS</TableHead>
                <TableHead className="rounded-none text-right">CPU</TableHead>
                <TableHead className="rounded-none text-right">REQS</TableHead>
                <TableHead className="rounded-none text-right">REQ/S</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {processes.length === 0 ? (
                <TableRow className="rounded-none">
                  <TableCell
                    colSpan={11}
                    className="rounded-none text-center text-foreground-muted py-6"
                  >
                    {error ? `[fetch error: ${error}]` : "no processes supervised for this app"}
                  </TableCell>
                </TableRow>
              ) : (
                processes.map((p) => (
                  <TableRow
                    key={p.process}
                    data-testid="app-detail-process-row"
                    className="rounded-none border-b border-line/30 last:border-b-0 hover:bg-white/5"
                  >
                    <TableCell className="rounded-none font-bold">{p.process}</TableCell>
                    <TableCell className="rounded-none text-right tabular">
                      {p.pid ?? "--"}
                    </TableCell>
                    <TableCell className="rounded-none">
                      <StatusTag status={p.status} />
                    </TableCell>
                    <TableCell
                      className={`rounded-none tabular ${
                        p.health === "healthy"
                          ? "text-accent-green"
                          : p.health === "unhealthy"
                            ? "text-accent-red"
                            : "text-foreground-dim"
                      }`}
                      title={p.healthDetail ?? undefined}
                    >
                      {p.health}
                      {p.healthDetail ? ` (${p.healthDetail})` : ""}
                      {p.lastHealthCheck ? (
                        <span className="text-foreground-dim"> · {formatAgo(p.lastHealthCheck)}</span>
                      ) : null}
                    </TableCell>
                    <TableCell className="rounded-none text-foreground-muted tabular">
                      {formatAgo(p.startedAt)}
                    </TableCell>
                    <TableCell className="rounded-none text-foreground-muted tabular">
                      {p.lastRestartAt ? formatAgo(p.lastRestartAt) : "--"}
                    </TableCell>
                    <TableCell className="rounded-none text-right tabular">
                      {p.restartCount}
                    </TableCell>
                    <TableCell className="rounded-none text-right tabular">
                      {formatBytes(p.rssBytes)}
                    </TableCell>
                    <TableCell className="rounded-none text-right tabular">
                      {formatCpuTime(p.cpuTimeMs)}
                    </TableCell>
                    <TableCell className="rounded-none text-right tabular">
                      {p.requestsTotal}
                    </TableCell>
                    <TableCell className="rounded-none text-right tabular">
                      {p.requestsPerSecond.toFixed(1)}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      </div>

      <NetworkPanel appFilter={app} />

      <div className="flex-1 grid grid-cols-1 lg:grid-cols-2 min-h-0">
        <div className="min-h-[24rem]">
          <TerminalStream app={app} />
        </div>
        <div className="flex flex-col min-h-[24rem]">
          <div className="flex items-center gap-2 border-b border-line px-3 py-2 shrink-0">
            <span className="font-display text-xs font-bold tracking-wide text-foreground-muted">
              ~/.riku/envs/{app}
            </span>
          </div>
          <EnvEditor app={app} />
        </div>
      </div>
    </div>
  );
}
