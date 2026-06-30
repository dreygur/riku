"use client";

import { useEffect, useRef, useState } from "react";
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
      <div className="flex items-center gap-4">
        <h1 className="font-mono text-sm tracking-widest text-muted-foreground uppercase">
          live metrics
        </h1>
        <span className="font-mono text-xs text-muted-foreground">
          total mem <span className="text-foreground">{fmtBytes(totalMem)}</span> · 2s sample
        </span>
      </div>

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
                  className="grid grid-cols-[140px_1fr_1fr] items-center gap-6 border-b border-border/50 px-4 py-3 font-mono text-xs last:border-b-0"
                >
                  <span className="font-bold">
                    {w.kind}.{w.ordinal}
                  </span>
                  <span className="flex items-center gap-3">
                    <span className="w-8 text-[10px] tracking-wider text-muted-foreground uppercase">
                      mem
                    </span>
                    <Sparkline data={h.mem} stroke="#5b9dd9" />
                    <span className="text-muted-foreground">{fmtBytes(w.memory_bytes)}</span>
                  </span>
                  <span className="flex items-center gap-3">
                    <span className="w-8 text-[10px] tracking-wider text-muted-foreground uppercase">
                      cpu
                    </span>
                    <Sparkline data={h.cpu} stroke="#e8b54a" />
                    <span className="text-muted-foreground">
                      {h.cpu.length ? `${h.cpu[h.cpu.length - 1]}ms/s` : "—"}
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
