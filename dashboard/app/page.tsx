"use client";

import * as React from "react";
import { SupervisorGrid } from "@/components/SupervisorGrid";
import { TerminalStream } from "@/components/TerminalStream";
import { EnvEditor } from "@/components/EnvEditor";
import { AppControls } from "@/components/AppControls";
import { PluginsPanel } from "@/components/PluginsPanel";
import { NetworkPanel } from "@/components/NetworkPanel";
import { api } from "@/lib/api";

export default function DashboardPage() {
  const [activeApp, setActiveApp] = React.useState("myapp");
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
    <div className="min-h-screen bg-background-dark text-foreground-dark font-mono flex flex-col">
      {/* ═══ TOP BAR ═══ */}
      <header className="flex items-center justify-between border-b border-primary-burgundy px-4 py-3 shrink-0">
        <h1 className="text-sm font-bold tracking-[0.2em] uppercase">
          RIKU // PLATFORM_DAEMON
        </h1>
        <div className="flex items-center gap-4 text-xs tabular">
          <span
            className={
              connStatus === "ok"
                ? "text-foreground-muted"
                : "text-accent-orange"
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
      <div className="border-b border-primary-burgundy">
        <SupervisorGrid />
      </div>

      {/* ═══ PLUGINS / HOOKS ═══ */}
      <PluginsPanel />

      {/* ═══ NETWORK / TLS ═══ */}
      <NetworkPanel />

      {/* ═══ BOTTOM PANELS ═══ */}
      <div className="flex-1 grid grid-cols-1 lg:grid-cols-2 min-h-0">
        {/* Terminal */}
        <div className="min-h-[24rem]">
          <TerminalStream app={activeApp} />
        </div>

        {/* Env Editor */}
        <div className="flex flex-col min-h-[24rem]">
          <div className="flex items-center gap-2 border-b border-primary-burgundy px-3 py-2 shrink-0">
            <span className="text-xs font-bold tracking-wide uppercase text-foreground-muted">
              ENV_CFG // {activeApp}
            </span>
          </div>
          <EnvEditor app={activeApp} />
        </div>
      </div>
    </div>
  );
}
