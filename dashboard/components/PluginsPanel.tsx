"use client";

import * as React from "react";
import { api } from "@/lib/api";

const POLL_INTERVAL_MS = 15_000;

export interface PluginsPanelProps {
  className?: string;
}

export function PluginsPanel({ className }: PluginsPanelProps) {
  const [clientPlugins, setClientPlugins] = React.useState<string[]>([]);
  const [hooks, setHooks] = React.useState<string[]>([]);
  const [error, setError] = React.useState<string | null>(null);
  const [installing, setInstalling] = React.useState(false);
  const [installMsg, setInstallMsg] = React.useState<string | null>(null);

  const fetchAll = React.useCallback(async () => {
    try {
      const [p, h] = await Promise.all([api.plugins.list(), api.hooks.list()]);
      setClientPlugins(p.plugins ?? []);
      setHooks(h.hooks ?? []);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "connection refused");
    }
  }, []);

  React.useEffect(() => {
    fetchAll();
    const id = setInterval(fetchAll, POLL_INTERVAL_MS);
    return () => clearInterval(id);
  }, [fetchAll]);

  const handleInstall = React.useCallback(async () => {
    setInstalling(true);
    setInstallMsg(null);
    try {
      const result = await api.control.installPlugins();
      setInstallMsg(result.ok ? "[INSTALL] ok" : `[INSTALL] failed: ${result.error ?? "unknown"}`);
      await fetchAll();
    } catch (e) {
      setInstallMsg(e instanceof Error ? e.message : "install failed");
    } finally {
      setInstalling(false);
    }
  }, [fetchAll]);

  return (
    <div className={`flex flex-col sm:flex-row border-b border-primary-burgundy ${className ?? ""}`}>
      {/* Client plugins */}
      <div className="flex-1 border-b sm:border-b-0 sm:border-r border-primary-burgundy/30">
        <div className="flex items-center justify-between px-3 py-2 border-b border-primary-burgundy/30">
          <span className="text-xs font-bold tracking-wide uppercase text-foreground-muted">
            CLIENT_PLUGINS //
          </span>
          <span className="text-xs text-foreground-dim tabular">
            {error ? <span className="text-accent-orange">[{error}]</span> : `${clientPlugins.length} installed`}
          </span>
        </div>
        <div className="px-3 py-2 text-sm">
          {clientPlugins.length === 0 ? (
            <span className="text-foreground-dim">none installed (~/.riku/client-plugins/)</span>
          ) : (
            <ul className="space-y-0.5">
              {clientPlugins.map((p) => (
                <li key={p} className="text-foreground-dark tabular">
                  {p}
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>

      {/* Server-side hook plugins */}
      <div className="flex-1">
        <div className="flex items-center justify-between px-3 py-2 border-b border-primary-burgundy/30">
          <span className="text-xs font-bold tracking-wide uppercase text-foreground-muted">
            HOOK_PLUGINS //
          </span>
          <div className="flex items-center gap-2">
            <span className="text-xs text-foreground-dim tabular">
              {hooks.length} installed
            </span>
            <button
              onClick={handleInstall}
              disabled={installing}
              className="rounded-none text-xs font-bold border border-primary-burgundy text-foreground-dark px-2 py-0.5 transition-colors hover:bg-accent-orange hover:text-background-dark hover:border-accent-orange disabled:opacity-40 disabled:cursor-not-allowed"
            >
              {installing ? "[INSTALLING...]" : "[INSTALL RUNTIMES]"}
            </button>
          </div>
        </div>
        <div className="px-3 py-2 text-sm">
          {hooks.length === 0 ? (
            <span className="text-foreground-dim">none installed (~/.riku/plugins/)</span>
          ) : (
            <ul className="space-y-0.5">
              {hooks.map((h) => (
                <li key={h} className="text-foreground-dark tabular">
                  {h}
                </li>
              ))}
            </ul>
          )}
          {installMsg && (
            <div className="mt-1 text-xs text-foreground-muted tabular">{installMsg}</div>
          )}
        </div>
      </div>
    </div>
  );
}
