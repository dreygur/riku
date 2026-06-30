"use client";

import { useCallback, useEffect, useState } from "react";
import { AppCard } from "@/components/riku/app-card";
import { api, fmtDur } from "@/lib/api";
import type { RikuState } from "@/lib/types";

export default function Overview() {
  const [state, setState] = useState<RikuState | null>(null);
  const [live, setLive] = useState(false);

  const load = useCallback(async () => {
    try {
      setState(await api.state());
      setLive(true);
    } catch {
      setLive(false);
    }
  }, []);

  useEffect(() => {
    load();
    const t = setInterval(load, 4000);
    return () => clearInterval(t);
  }, [load]);

  return (
    <div className="min-h-screen">
      <header className="sticky top-0 z-10 flex items-center gap-5 border-b border-border bg-background/85 px-5 py-3.5 backdrop-blur">
        <div className="font-mono text-lg font-bold">
          riku<span className="text-primary motion-safe:animate-pulse">▌</span>
        </div>
        <div className="font-mono text-xs text-muted-foreground">
          {state ? (
            <>
              <span className="text-foreground">v{state.riku_version}</span> · up{" "}
              {fmtDur(state.supervisor_uptime_seconds)} ·{" "}
              <span className="text-foreground">{state.apps.length}</span>{" "}
              app{state.apps.length === 1 ? "" : "s"}
            </>
          ) : (
            "connecting…"
          )}
        </div>
        <div className="flex-1" />
        <span
          className={`h-1.5 w-1.5 ${live ? "bg-primary motion-safe:animate-pulse" : "bg-muted-foreground/40"}`}
          title={live ? "live" : "offline"}
        />
      </header>

      <main className="mx-auto max-w-5xl space-y-4 px-5 py-7">
        {!state ? (
          <p className="py-20 text-center font-mono text-sm text-muted-foreground">
            reading the supervisor…
          </p>
        ) : state.apps.length === 0 ? (
          <div className="py-20 text-center text-muted-foreground">
            No apps deployed yet.
            <br />
            <br />
            Push one:{" "}
            <code className="bg-card px-2 py-1 font-mono text-primary">git push riku main</code>
          </div>
        ) : (
          state.apps.map((a) => <AppCard key={a.app} app={a} onChanged={load} />)
        )}
      </main>
    </div>
  );
}
