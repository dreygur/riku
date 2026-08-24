"use client";

import { useEffect, useRef, useState } from "react";
import { PageHeader } from "@/components/riku/page-header";
import { Sparkline } from "@/components/riku/sparkline";
import { api, fmtBytes } from "@/lib/api";
import type { RikuState } from "@/lib/types";

const WINDOW = 60; // ~2 min at 2s

type Sample = { mem: number[]; cpu: number[] }; // cpu = per-interval delta ms
type History = Record<string, Sample>;

export default function MetricsPage() {
  const [state, setState] = useState<RikuState | null>(null);
  const [, force] = useState(0);
  const hist = useRef<History>({});
  const lastCpu = useRef<Record<string, number>>({});

  useEffect(() => {
    const tick = async () => {
      let s: RikuState;
      try {
        s = await api.state();
      } catch {
        return;
      }
      for (const app of s.apps) {
        for (const w of app.workers) {
          const h = (hist.current[w.process_id] ??= { mem: [], cpu: [] });
          h.mem.push(w.memory_bytes || 0);
          const prev = lastCpu.current[w.process_id];
          const delta = prev === undefined ? 0 : Math.max(0, (w.cpu_time_ms || 0) - prev);
          lastCpu.current[w.process_id] = w.cpu_time_ms || 0;
          h.cpu.push(delta);
          if (h.mem.length > WINDOW) h.mem.shift();
          if (h.cpu.length > WINDOW) h.cpu.shift();
        }
      }
      setState(s);
      force((n) => n + 1);
    };
    tick();
    const t = setInterval(tick, 2000);
    return () => clearInterval(t);
  }, []);

  if (!state) {
    return <p className="py-20 text-center font-mono text-sm text-muted-foreground">collecting metrics…</p>;
  }

  const totalMem = state.apps
    .flatMap((a) => a.workers)
    .reduce((sum, w) => sum + (w.memory_bytes || 0), 0);

  return (
    <div className="space-y-6">
      <PageHeader
        title="live metrics"
        meta={
          <span className="font-mono text-xs text-muted-foreground">
            total mem <span className="text-foreground">{fmtBytes(totalMem)}</span> · 2s sample
          </span>
        }
      />

      {state.apps.map((app) => (
        <section key={app.app} className="border border-border bg-card">
          <div className="border-b border-border px-4 py-2.5 font-mono text-sm font-bold">
            {app.app}
          </div>
          {app.workers.length === 0 ? (
            <p className="px-4 py-4 font-mono text-xs text-muted-foreground">no workers</p>
          ) : (
            app.workers.map((w) => {
              const h = hist.current[w.process_id] ?? { mem: [], cpu: [] };
              return (
                <div
                  key={w.process_id}
                  className="flex flex-col gap-2 border-b border-border/50 px-4 py-3 font-mono text-xs last:border-b-0 sm:grid sm:grid-cols-[140px_1fr_1fr] sm:items-center sm:gap-6"
                >
                  <span className="font-bold">
                    {w.kind}.{w.ordinal}
                  </span>
                  <span className="flex min-w-0 items-center gap-3">
                    <span className="w-8 shrink-0 text-[10px] tracking-wider text-muted-foreground uppercase">
                      mem
                    </span>
                    <Sparkline data={h.mem} stroke="var(--color-info)" />
                    <span className="shrink-0 text-muted-foreground">{fmtBytes(w.memory_bytes)}</span>
                  </span>
                  <span className="flex min-w-0 items-center gap-3">
                    <span className="w-8 shrink-0 text-[10px] tracking-wider text-muted-foreground uppercase">
                      cpu
                    </span>
                    <Sparkline data={h.cpu} stroke="var(--color-warn)" />
                    <span className="shrink-0 text-muted-foreground">
                      {h.cpu.length ? `${h.cpu[h.cpu.length - 1]}ms/s` : "-"}
                    </span>
                  </span>
                </div>
              );
            })
          )}
        </section>
      ))}
    </div>
  );
}
