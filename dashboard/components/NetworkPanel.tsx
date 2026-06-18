"use client";

import * as React from "react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { api } from "@/lib/api";
import { mapBackendToNetwork, type NetworkInfo } from "@/lib/types";

const POLL_INTERVAL_MS = 30_000;

export interface NetworkPanelProps {
  className?: string;
}

function tlsStatus(tlsExpiry: string | null): { label: string; className: string } {
  if (!tlsExpiry) return { label: "NO CERT", className: "text-foreground-dim" };

  const expires = new Date(tlsExpiry).getTime();
  if (Number.isNaN(expires)) return { label: "UNKNOWN", className: "text-foreground-dim" };

  const daysLeft = Math.floor((expires - Date.now()) / (1000 * 60 * 60 * 24));
  if (daysLeft < 0) return { label: "EXPIRED", className: "text-accent-orange font-bold" };
  if (daysLeft <= 14) return { label: `${daysLeft}D LEFT`, className: "text-accent-orange" };
  return { label: `${daysLeft}D LEFT`, className: "text-foreground-muted" };
}

export function NetworkPanel({ className }: NetworkPanelProps) {
  const [entries, setEntries] = React.useState<NetworkInfo[]>([]);
  const [error, setError] = React.useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = React.useState<Date | null>(null);

  const fetchNetwork = React.useCallback(async () => {
    try {
      const data = await api.network.list();
      setEntries(mapBackendToNetwork(data.apps ?? []));
      setLastUpdated(new Date());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "connection refused");
    }
  }, []);

  React.useEffect(() => {
    fetchNetwork();
    const id = setInterval(fetchNetwork, POLL_INTERVAL_MS);
    return () => clearInterval(id);
  }, [fetchNetwork]);

  return (
    <div className={`border-b border-primary-burgundy ${className ?? ""}`}>
      <div className="flex items-center justify-between border-b border-primary-burgundy/30 px-3 py-2">
        <span className="text-xs font-bold tracking-wide uppercase text-foreground-muted">
          NETWORK_TLS //
        </span>
        <span className="text-xs text-foreground-muted tabular">
          {error ? (
            <span className="text-accent-orange">[{error}]</span>
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
            <TableRow className="rounded-none border-b border-primary-burgundy/30">
              <TableHead className="rounded-none">APP</TableHead>
              <TableHead className="rounded-none">SERVER_NAME</TableHead>
              <TableHead className="rounded-none">UPSTREAM</TableHead>
              <TableHead className="rounded-none text-right">TLS</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {entries.length === 0 ? (
              <TableRow className="rounded-none">
                <TableCell
                  colSpan={4}
                  className="rounded-none text-center text-foreground-muted py-6"
                >
                  {error ? `[fetch error: ${error}]` : "no nginx vhosts configured"}
                </TableCell>
              </TableRow>
            ) : (
              entries.map((e) => {
                const tls = tlsStatus(e.tlsExpiry);
                return (
                  <TableRow
                    key={e.app}
                    className="rounded-none border-b border-primary-burgundy/30 last:border-b-0 hover:bg-white/5"
                  >
                    <TableCell className="rounded-none font-bold">{e.app}</TableCell>
                    <TableCell className="rounded-none text-foreground-muted tabular">
                      {e.serverName ?? "--"}
                    </TableCell>
                    <TableCell className="rounded-none text-foreground-muted tabular">
                      {e.upstream ?? "--"}
                    </TableCell>
                    <TableCell className={`rounded-none text-right tabular ${tls.className}`}>
                      [{tls.label}]
                    </TableCell>
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}
