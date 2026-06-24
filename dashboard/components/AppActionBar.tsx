"use client";

import * as React from "react";
import { api } from "@/lib/api";
import type { ActionKey } from "@/lib/useAppActions";
import type { ControlActionResponse } from "@/lib/api";

export interface AppActionBarProps {
  app: string;
  pending: ActionKey | null;
  message: string | null;
  isError: boolean;
  runAction: (
    action: ActionKey,
    fn: () => Promise<ControlActionResponse & { output?: string }>,
  ) => void | Promise<void>;
  className?: string;
}

const buttonClass =
  "rounded-none text-xs font-bold border border-line px-2 py-0.5 transition-colors disabled:opacity-40 disabled:cursor-not-allowed";

/** The deploy/restart/stop/destroy/export button row + status message,
 * shared between the app-switcher controls and the per-app detail page.
 * Takes `useAppActions(app)`'s state as props so callers that also drive
 * other actions (e.g. AppControls' create flow) share one status line. */
export function AppActionBar({
  app,
  pending,
  message,
  isError,
  runAction,
  className,
}: AppActionBarProps) {
  return (
    <div className={`flex flex-wrap items-center gap-2 ${className ?? ""}`}>
      <button
        data-testid="deploy-btn"
        onClick={() => runAction("deploy", () => api.control.deploy(app))}
        disabled={pending !== null || !app}
        className={`${buttonClass} text-foreground-dark hover:bg-accent-amber hover:text-background-dark hover:border-accent-amber`}
      >
        {pending === "deploy" ? "[DEPLOYING...]" : "[DEPLOY]"}
      </button>

      <button
        data-testid="restart-btn"
        onClick={() => runAction("restart", () => api.control.restart(app))}
        disabled={pending !== null || !app}
        className={`${buttonClass} text-foreground-muted hover:bg-line/10 hover:text-foreground-dark`}
      >
        {pending === "restart" ? "[RESTARTING...]" : "[RESTART]"}
      </button>

      <button
        data-testid="stop-btn"
        onClick={() => runAction("stop", () => api.control.stop(app))}
        disabled={pending !== null || !app}
        className={`${buttonClass} text-foreground-muted hover:bg-line/10 hover:text-foreground-dark`}
      >
        {pending === "stop" ? "[STOPPING...]" : "[STOP]"}
      </button>

      <button
        data-testid="destroy-btn"
        onClick={() => runAction("destroy", () => api.control.destroy(app))}
        disabled={pending !== null || !app}
        className={`${buttonClass} text-accent-red hover:bg-accent-red hover:text-background-dark hover:border-accent-red`}
      >
        {pending === "destroy" ? "[DESTROYING...]" : "[DESTROY]"}
      </button>

      <button
        data-testid="export-image-btn"
        onClick={() => runAction("container_export", () => api.control.containerExport(app))}
        disabled={pending !== null || !app}
        title="Builds the app's deployed source as a container image (Docker/Podman) and exports it to a tar archive on the server"
        className={`${buttonClass} text-foreground-muted hover:bg-line/10 hover:text-foreground-dark`}
      >
        {pending === "container_export" ? "[EXPORTING...]" : "[EXPORT IMAGE]"}
      </button>

      {message && (
        <span
          data-testid="action-status"
          data-error={isError}
          className={`text-xs tabular ml-2 ${isError ? "text-accent-red" : "text-foreground-muted"}`}
        >
          {message}
        </span>
      )}
    </div>
  );
}
