"use client";

import * as React from "react";
import { SupervisorGrid } from "@/components/SupervisorGrid";
import { TerminalStream } from "@/components/TerminalStream";
import { EnvEditor } from "@/components/EnvEditor";
import { AppControls } from "@/components/AppControls";
import { PluginsPanel } from "@/components/PluginsPanel";
import { NetworkPanel } from "@/components/NetworkPanel";
import { CollapsibleSection } from "@/components/CollapsibleSection";
import { api } from "@/lib/api";

const PIPELINE_STAGES = [
  { code: "REPO", label: "git push -> repos/" },
  { code: "BUILD", label: "plugin build" },
  { code: "RUN", label: "supervisor exec" },
  { code: "ROUTE", label: "nginx + tls" },
] as const;

function PipelineRail({ live }: { live: boolean }) {
  return (
    <aside
      aria-hidden
      className="hidden md:flex w-9 flex-col shrink-0 border-r border-line"
    >
      {PIPELINE_STAGES.map((stage) => (
        <div
          key={stage.code}
          title={stage.label}
          className="flex-1 flex items-center justify-center border-b border-line last:border-b-0"
        >
          <span
            className={`font-display text-[10px] font-bold tracking-[0.3em] [writing-mode:vertical-rl] rotate-180 ${
              live ? "text-accent-amber" : "text-foreground-dim"
            }`}
          >
            {stage.code}
          </span>
        </div>
      ))}
    </aside>
  );
}

export default function DashboardPage() {
  const [activeApp, setActiveApp] = React.useState("");
  const [connStatus, setConnStatus] = React.useState<"ok" | "offline">(
    "offline",
  );
  const [uptime, setUptime] = React.useState<number | null>(null);

  React.useEffect(() => {
    const poll = async () => {
      try {
        const h = await api.health.get();
        setConnStatus("ok");
        setUptime(h.uptime);
      } catch {
        setConnStatus("offline");
      }
    };
    poll();
    const id = setInterval(poll, 10_000);
    return () => clearInterval(id);
  }, []);

  const fmtUptime = (s: number) => {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    return `${h}h${String(m).padStart(2, "0")}m`;
  };

  return (
    <div className="min-h-screen bg-background-dark text-foreground-dark font-mono flex">
      <PipelineRail live={connStatus === "ok"} />

      <div className="flex-1 flex flex-col min-w-0">
        {/* ═══ TOP BAR ═══ */}
        <header className="flex items-center justify-between border-b border-line px-4 py-3 shrink-0">
          <h1 className="font-display text-sm font-bold tracking-[0.2em] uppercase">
            RIKU // PLATFORM_DAEMON
          </h1>
          <div className="flex items-center gap-4 text-xs tabular">
            <span
              className={
                connStatus === "ok" ? "text-accent-green" : "text-accent-red"
              }
            >
              [CONN:{connStatus === "ok" ? "OK" : "OFFLINE"}]
            </span>
            <span className="text-foreground-dim">
              {uptime !== null ? `UP:${fmtUptime(uptime)}` : "UP:--"}
            </span>
          </div>
        </header>

        {/* ═══ APP CONTROLS ═══ */}
        <AppControls app={activeApp} onAppChange={setActiveApp} />

        {/* ═══ SUPERVISOR GRID ═══ */}
        <div className="border-b border-line">
          <SupervisorGrid />
        </div>

        {/* ═══ RUNTIME CONFIG (plugins / hooks / nginx) ═══
            Low-churn relative to the worker grid and logs below, so it's
            collapsed by default to keep those primary, frequently-checked
            panels closer to the top of the page. */}
        <CollapsibleSection title="runtime config (plugins, hooks, nginx routes)" storageKey="runtime-config">
          <PluginsPanel />
          <NetworkPanel />
        </CollapsibleSection>

        {/* ═══ BOTTOM PANELS ═══ */}
        <div className="flex-1 grid grid-cols-1 lg:grid-cols-2 min-h-0">
          {/* Terminal */}
          <div className="min-h-[24rem]">
            <TerminalStream app={activeApp} />
          </div>

          {/* Env Editor */}
          <div className="flex flex-col min-h-[24rem]">
            <div className="flex items-center gap-2 border-b border-line px-3 py-2 shrink-0">
              <span className="font-display text-xs font-bold tracking-wide text-foreground-muted">
                ~/.riku/envs/{activeApp}
              </span>
            </div>
            <EnvEditor app={activeApp} />
          </div>
        </div>
      </div>
    </div>
  );
}
