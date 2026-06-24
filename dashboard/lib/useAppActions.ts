"use client";

import * as React from "react";
import { api } from "@/lib/api";
import type { ControlActionResponse } from "@/lib/api";

export type ActionKey =
  | "create"
  | "deploy"
  | "restart"
  | "stop"
  | "destroy"
  | "container_export";

const CONFIRM_MESSAGE: Partial<Record<ActionKey, string>> = {
  destroy: "Destroy app",
};

/** Shared "run a control-plane action against an app, report ok/error" state
 * machine used by both the app-switcher controls and the per-app action bar. */
export function useAppActions(app: string) {
  const [pending, setPending] = React.useState<ActionKey | null>(null);
  const [message, setMessage] = React.useState<string | null>(null);
  const [isError, setIsError] = React.useState(false);

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
      if (
        CONFIRM_MESSAGE[action] &&
        !window.confirm(`${CONFIRM_MESSAGE[action]} '${app}'? This cannot be undone.`)
      ) {
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

  return { pending, message, isError, runAction, report, setPending, setMessage };
}
