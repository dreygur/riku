"use client";

import { useEffect, useState } from "react";
import { StatusDot } from "@/components/riku/status-dot";
import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import type { DoctorCheck } from "@/lib/types";

const dotFor = (s: DoctorCheck["status"]) =>
  s === "ok" ? "alive" : s === "warn" ? "warn" : "dead";

export default function DoctorPage() {
  const [checks, setChecks] = useState<DoctorCheck[] | null>(null);
  const [loading, setLoading] = useState(false);

  const run = async () => {
    setLoading(true);
    try {
      setChecks(await api.doctor());
    } catch {
      setChecks([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    run();
  }, []);

  const counts = (checks ?? []).reduce(
    (acc, c) => ((acc[c.status] = (acc[c.status] ?? 0) + 1), acc),
    {} as Record<string, number>,
  );

  return (
    <div>
      <div className="mb-5 flex items-center gap-4">
        <h1 className="font-mono text-sm tracking-widest text-muted-foreground uppercase">
          system diagnostics
        </h1>
        {checks && (
          <span className="font-mono text-xs text-muted-foreground">
            <span className="text-[#3fd07f]">{counts.ok ?? 0} ok</span> ·{" "}
            <span className="text-[#e8b54a]">{counts.warn ?? 0} warn</span> ·{" "}
            <span className="text-[#f25f5c]">{counts.fail ?? 0} fail</span>
          </span>
        )}
        <span className="flex-1" />
        <Button size="sm" variant="secondary" disabled={loading} onClick={run}>
          {loading ? "running…" : "re-run"}
        </Button>
      </div>

      <div className="border border-border bg-card">
        {!checks ? (
          <p className="px-4 py-8 text-center font-mono text-xs text-muted-foreground">
            running checks…
          </p>
        ) : (
          checks.map((c) => (
            <div
              key={c.name}
              className="grid grid-cols-[16px_140px_1fr] items-start gap-3 border-b border-border/50 px-4 py-3 font-mono text-xs last:border-b-0"
            >
              <StatusDot kind={dotFor(c.status)} className="mt-0.5" />
              <span className="font-bold text-foreground">{c.name}</span>
              <span className="text-muted-foreground">{c.detail}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
