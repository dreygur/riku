"use client";

import * as React from "react";
import Link from "next/link";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { api } from "@/lib/api";
import { type WorkerInfo, mapBackendToWorkers } from "@/lib/types";
import { formatAgo, formatBytes, formatCpuTime } from "@/lib/format";
import { StatusTag } from "@/components/StatusTag";

const POLL_INTERVAL_MS = 5_000;

/** Tooltip text surfacing the fields too dense for the grid's columns —
 * the per-app detail page is where these get their own columns. */
function workerTooltip(w: WorkerInfo): string {
  const parts = [
    `health: ${w.health}${w.healthDetail ? ` (${w.healthDetail})` : ""}`,
    `started: ${formatAgo(w.startedAt)}`,
  ];
  if (w.lastRestartAt) parts.push(`last restart: ${formatAgo(w.lastRestartAt)}`);
  return parts.join(" | ");
}

export interface SupervisorGridProps {
  pollInterval?: number;
  className?: string;
}

export function SupervisorGrid({
  pollInterval = POLL_INTERVAL_MS,
  className,
}: SupervisorGridProps) {
  const [workers, setWorkers] = React.useState<WorkerInfo[]>([]);
  const [error, setError] = React.useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = React.useState<Date | null>(null);

  const fetchMetrics = React.useCallback(async () => {
    try {
      const data = await api.metrics.get();
      setWorkers(mapBackendToWorkers(data));
      setLastUpdated(new Date());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "connection refused");
    }
  }, []);

  React.useEffect(() => {
    fetchMetrics();
    const id = setInterval(fetchMetrics, pollInterval);
    return () => clearInterval(id);
  }, [fetchMetrics, pollInterval]);

  const sorted = React.useMemo(
    () =>
      [...workers].sort((a, b) =>
        a.app === b.app
          ? a.process.localeCompare(b.process)
          : a.app.localeCompare(b.app),
      ),
    [workers],
  );

  return (
    <div className={className} data-testid="supervisor-grid">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-line px-3 py-2">
        <span className="font-display text-xs font-bold tracking-wide text-foreground-muted">
          ~/.riku/workers-enabled
        </span>
        <span data-testid="supervisor-grid-status" className="text-xs text-foreground-muted tabular">
          {error ? (
            <span className="text-accent-red">[{error}]</span>
          ) : lastUpdated ? (
            `updated ${lastUpdated.toLocaleTimeString()}`
          ) : (
            "loading..."
          )}
        </span>
      </div>

      {/* Data table */}
      <div className="overflow-x-auto">
        <Table className="rounded-none text-sm">
          <TableHeader>
            <TableRow className="rounded-none border-b border-line/30">
              <TableHead className="rounded-none">APP</TableHead>
              <TableHead className="rounded-none">PROC</TableHead>
              <TableHead className="rounded-none text-right">PID</TableHead>
              <TableHead className="rounded-none text-right">
                RSS MEMORY
              </TableHead>
              <TableHead className="rounded-none text-right">CPU %</TableHead>
              <TableHead className="rounded-none text-right">REQ/S</TableHead>
              <TableHead className="rounded-none text-right">
                RESTARTS
              </TableHead>
              <TableHead className="rounded-none">STATUS</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {sorted.length === 0 ? (
              <TableRow className="rounded-none">
                <TableCell
                  colSpan={8}
                  className="rounded-none text-center text-foreground-muted py-6"
                >
                  {error ? `[fetch error: ${error}]` : "no workers supervised"}
                </TableCell>
              </TableRow>
            ) : (
              sorted.map((w) => (
                <TableRow
                  key={`${w.app}:${w.process}`}
                  data-testid="worker-row"
                  data-app={w.app}
                  data-process={w.process}
                  data-status={w.status}
                  data-pid={w.pid ?? ""}
                  title={workerTooltip(w)}
                  className="rounded-none border-b border-line/30 last:border-b-0 hover:bg-white/5"
                >
                  <TableCell className="rounded-none font-bold">
                    <Link
                      href={`/apps/${encodeURIComponent(w.app)}`}
                      className="hover:text-accent-amber hover:underline"
                    >
                      {w.app}
                    </Link>
                  </TableCell>
                  <TableCell className="rounded-none text-foreground-muted">
                    {w.process}
                  </TableCell>
                  <TableCell className="rounded-none text-right tabular">
                    {w.pid ?? "--"}
                  </TableCell>
                  <TableCell className="rounded-none text-right tabular">
                    {formatBytes(w.rssBytes)}
                  </TableCell>
                  <TableCell className="rounded-none text-right tabular">
                    {formatCpuTime(w.cpuTimeMs)}
                  </TableCell>
                  <TableCell className="rounded-none text-right tabular">
                    {w.requestsPerSecond.toFixed(1)}
                  </TableCell>
                  <TableCell className="rounded-none text-right tabular">
                    {w.restartCount}
                  </TableCell>
                  <TableCell className="rounded-none">
                    <StatusTag status={w.status} />
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}
