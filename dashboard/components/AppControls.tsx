"use client";

import * as React from "react";
import { api } from "@/lib/api";
import type { ControlActionResponse } from "@/lib/api";

export interface AppControlsProps {
  app: string;
  onAppChange: (app: string) => void;
  className?: string;
}

type ActionKey =
  | "create"
  | "deploy"
  | "restart"
  | "stop"
  | "destroy"
  | "container_export";

const CONFIRM_MESSAGE: Partial<Record<ActionKey, string>> = {
  destroy: "Destroy app",
};

export function AppControls({ app, onAppChange, className }: AppControlsProps) {
  const [pending, setPending] = React.useState<ActionKey | null>(null);
  const [message, setMessage] = React.useState<string | null>(null);
  const [isError, setIsError] = React.useState(false);
  const [newAppName, setNewAppName] = React.useState("");

  const report = React.useCallback(
    (
      action: ActionKey,
      result: (ControlActionResponse & { output?: string }) | null,
      err: unknown,
    ) => {
      if (err) {
        setIsError(true);
        setMessage(err instanceof Error ? err.message : String(err));
        return;
      }
      setIsError(!result?.ok);
      setMessage(
        result?.ok
          ? `[${action.toUpperCase()}] ${result.app ?? app} ok${result.output ? ` -> ${result.output}` : ""}`
          : `[${action.toUpperCase()}] failed: ${result?.error ?? "unknown error"}`,
      );
    },
    [app],
  );

  const runAction = React.useCallback(
    async (
      action: ActionKey,
      fn: () => Promise<ControlActionResponse & { output?: string }>,
    ) => {
      if (CONFIRM_MESSAGE[action] && !window.confirm(`${CONFIRM_MESSAGE[action]} '${app}'? This cannot be undone.`)) {
        return;
      }
      setPending(action);
      setMessage(null);
      try {
        const result = await fn();
        report(action, result, null);
      } catch (e) {
        report(action, null, e);
      } finally {
        setPending(null);
      }
    },
    [app, report],
  );

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
  }, [newAppName, onAppChange, report]);

  const buttonClass =
    "rounded-none text-xs font-bold border border-primary-burgundy px-2 py-0.5 transition-colors disabled:opacity-40 disabled:cursor-not-allowed";

  return (
    <div
      className={`flex flex-wrap items-center gap-2 border-b border-primary-burgundy px-3 py-2 ${className ?? ""}`}
    >
      <span className="text-xs font-bold tracking-wide uppercase text-foreground-muted">
        APP_CTL //
      </span>

      <input
        type="text"
        value={app}
        onChange={(e) => onAppChange(e.target.value)}
        className="rounded-none bg-transparent border border-primary-burgundy px-2 py-0.5 text-xs text-foreground-dark outline-none focus:border-accent-orange tabular w-32"
      />

      <button
        onClick={() => runAction("deploy", () => api.control.deploy(app))}
        disabled={pending !== null || !app}
        className={`${buttonClass} text-foreground-dark hover:bg-accent-orange hover:text-background-dark hover:border-accent-orange`}
      >
        {pending === "deploy" ? "[DEPLOYING...]" : "[DEPLOY]"}
      </button>

      <button
        onClick={() => runAction("restart", () => api.control.restart(app))}
        disabled={pending !== null || !app}
        className={`${buttonClass} text-foreground-muted hover:bg-primary-burgundy/10 hover:text-foreground-dark`}
      >
        {pending === "restart" ? "[RESTARTING...]" : "[RESTART]"}
      </button>

      <button
        onClick={() => runAction("stop", () => api.control.stop(app))}
        disabled={pending !== null || !app}
        className={`${buttonClass} text-foreground-muted hover:bg-primary-burgundy/10 hover:text-foreground-dark`}
      >
        {pending === "stop" ? "[STOPPING...]" : "[STOP]"}
      </button>

      <button
        onClick={() => runAction("destroy", () => api.control.destroy(app))}
        disabled={pending !== null || !app}
        className={`${buttonClass} text-accent-orange hover:bg-accent-orange hover:text-background-dark hover:border-accent-orange`}
      >
        {pending === "destroy" ? "[DESTROYING...]" : "[DESTROY]"}
      </button>

      <button
        onClick={() => runAction("container_export", () => api.control.containerExport(app))}
        disabled={pending !== null || !app}
        title="Builds the app's deployed source as a container image (Docker/Podman) and exports it to a tar archive on the server"
        className={`${buttonClass} text-foreground-muted hover:bg-primary-burgundy/10 hover:text-foreground-dark`}
      >
        {pending === "container_export" ? "[EXPORTING...]" : "[EXPORT IMAGE]"}
      </button>

      <span className="mx-1 text-foreground-dim">|</span>

      <input
        type="text"
        placeholder="new app name"
        value={newAppName}
        onChange={(e) => setNewAppName(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && newAppName.trim()) handleCreate();
        }}
        className="rounded-none bg-transparent border border-primary-burgundy px-2 py-0.5 text-xs text-foreground-dark placeholder:text-foreground-dim outline-none focus:border-accent-orange w-36"
      />
      <button
        onClick={handleCreate}
        disabled={pending !== null || !newAppName.trim()}
        className={`${buttonClass} text-foreground-dark hover:bg-accent-orange hover:text-background-dark hover:border-accent-orange`}
      >
        {pending === "create" ? "[CREATING...]" : "[CREATE]"}
      </button>

      {message && (
        <span className={`text-xs tabular ml-2 ${isError ? "text-accent-orange" : "text-foreground-muted"}`}>
          {message}
        </span>
      )}
    </div>
  );
}
