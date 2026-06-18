"use client";

import * as React from "react";
import { api } from "@/lib/api";
import { parseUpstreamPort } from "@/lib/format";
import { useAppActions } from "@/lib/useAppActions";
import { AppActionBar } from "@/components/AppActionBar";

const APPS_POLL_INTERVAL_MS = 15_000;

export interface AppControlsProps {
  app: string;
  onAppChange: (app: string) => void;
  className?: string;
}

export function AppControls({ app, onAppChange, className }: AppControlsProps) {
  const [newAppName, setNewAppName] = React.useState("");
  const [knownApps, setKnownApps] = React.useState<string[]>([]);
  const [port, setPort] = React.useState<string | null>(null);
  const { pending, message, isError, runAction, report, setPending, setMessage } =
    useAppActions(app);

  React.useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      try {
        const [stats, network] = await Promise.all([
          api.metrics.get(),
          api.network.list(),
        ]);
        if (cancelled) return;
        setKnownApps((prev) => {
          const names = Array.from(new Set(stats.map((s) => s.app))).sort();
          return names.length === prev.length &&
            names.every((n, i) => n === prev[i])
            ? prev
            : names;
        });
        const entry = network.apps?.find((n) => n.app === app);
        setPort(parseUpstreamPort(entry?.upstream ?? null));
      } catch {
        // Leave the last known app list / port in place; the panels below
        // already surface connection errors.
      }
    };
    poll();
    const id = setInterval(poll, APPS_POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [app]);

  const handleCreate = React.useCallback(async () => {
    const name = newAppName.trim();
    if (!name) return;
    setPending("create");
    setMessage(null);
    try {
      const result = await api.control.create(name);
      report("create", result, null);
      if (result.ok) {
        onAppChange(name);
        setNewAppName("");
      }
    } catch (e) {
      report("create", null, e);
    } finally {
      setPending(null);
    }
  }, [newAppName, onAppChange, report, setPending, setMessage]);

  const buttonClass =
    "rounded-none text-xs font-bold border border-line px-2 py-0.5 transition-colors disabled:opacity-40 disabled:cursor-not-allowed";

  return (
    <div
      className={`flex flex-wrap items-center gap-2 border-b border-line px-3 py-2 ${className ?? ""}`}
    >
      <span className="font-display text-xs font-bold tracking-wide text-foreground-muted">
        ~/.riku/repos/
      </span>

      <input
        type="text"
        data-testid="active-app-input"
        value={app}
        onChange={(e) => onAppChange(e.target.value)}
        list="known-apps"
        placeholder="app name"
        title={knownApps.length > 0 ? `known apps: ${knownApps.join(", ")}` : undefined}
        className="rounded-none bg-transparent border border-line px-2 py-0.5 text-xs text-foreground-dark outline-none focus:border-accent-amber tabular w-32"
      />
      <datalist id="known-apps">
        {knownApps.map((name) => (
          <option key={name} value={name} />
        ))}
      </datalist>

      <span
        data-testid="active-app-port"
        className="text-xs tabular text-foreground-muted"
        title="resolved nginx upstream port for the selected app"
      >
        [PORT {port ?? "--"}]
      </span>

      <AppActionBar
        app={app}
        pending={pending}
        message={message}
        isError={isError}
        runAction={runAction}
      />

      <span className="mx-1 text-foreground-dim">|</span>

      <input
        type="text"
        data-testid="new-app-input"
        placeholder="new app name"
        value={newAppName}
        onChange={(e) => setNewAppName(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && newAppName.trim()) handleCreate();
        }}
        className="rounded-none bg-transparent border border-line px-2 py-0.5 text-xs text-foreground-dark placeholder:text-foreground-dim outline-none focus:border-accent-amber w-36"
      />
      <button
        data-testid="create-btn"
        onClick={handleCreate}
        disabled={pending !== null || !newAppName.trim()}
        className={`${buttonClass} text-foreground-dark hover:bg-accent-amber hover:text-background-dark hover:border-accent-amber`}
      >
        {pending === "create" ? "[CREATING...]" : "[CREATE]"}
      </button>
    </div>
  );
}
