"use client";

import * as React from "react";
import { api } from "@/lib/api";
import type { EnvVar } from "@/lib/types";

export interface EnvEditorProps {
  app: string;
  className?: string;
}

export function EnvEditor({ app, className }: EnvEditorProps) {
  const [vars, setVars] = React.useState<EnvVar[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);

  const [editingKey, setEditingKey] = React.useState<string | null>(null);
  const [editValue, setEditValue] = React.useState("");

  const [newKey, setNewKey] = React.useState("");
  const [newValue, setNewValue] = React.useState("");
  const [adding, setAdding] = React.useState(false);

  const fetchEnv = React.useCallback(async () => {
    try {
      setLoading(true);
      const data = await api.env.list(app);
      setVars(data.vars ?? []);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "connection refused");
    } finally {
      setLoading(false);
    }
  }, [app]);

  React.useEffect(() => {
    fetchEnv();
  }, [fetchEnv]);

  const handleUpdate = React.useCallback(
    async (key: string, value: string) => {
      try {
        await api.env.set(app, key, value);
        setVars((prev) =>
          prev.map((v) => (v.key === key ? { key, value } : v)),
        );
        setEditingKey(null);
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : "update failed");
      }
    },
    [app],
  );

  const handleDelete = React.useCallback(
    async (key: string) => {
      try {
        await api.env.delete(app, key);
        setVars((prev) => prev.filter((v) => v.key !== key));
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : "delete failed");
      }
    },
    [app],
  );

  const handleAdd = React.useCallback(async () => {
    const trimmedKey = newKey.trim();
    if (!trimmedKey) return;
    setAdding(true);
    try {
      await api.env.set(app, trimmedKey, newValue);
      setVars((prev) => [...prev, { key: trimmedKey, value: newValue }]);
      setNewKey("");
      setNewValue("");
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "add failed");
    } finally {
      setAdding(false);
    }
  }, [app, newKey, newValue]);

  return (
    <div className={`flex flex-col h-full ${className ?? ""}`}>
      {/* Error bar */}
      {error && (
        <div className="border-b border-primary-burgundy px-3 py-1.5 shrink-0">
          <span className="text-xs text-accent-orange tabular">[{error}]</span>
        </div>
      )}

      {/* Data grid */}
      <div className="flex-1 overflow-y-auto min-h-0">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-primary-burgundy/30">
              <th className="rounded-none text-left px-3 py-2 text-xs font-bold tracking-wide uppercase text-foreground-muted">
                KEY
              </th>
              <th className="rounded-none text-left px-3 py-2 text-xs font-bold tracking-wide uppercase text-foreground-muted">
                VALUE
              </th>
              <th className="rounded-none w-24 px-3 py-2" />
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr>
                <td
                  colSpan={3}
                  className="text-center py-6 text-foreground-dim"
                >
                  loading...
                </td>
              </tr>
            ) : vars.length === 0 ? (
              <tr>
                <td
                  colSpan={3}
                  className="text-center py-6 text-foreground-dim"
                >
                  no environment variables
                </td>
              </tr>
            ) : (
              vars.map((v) => (
                <tr
                  key={v.key}
                  className="border-b border-primary-burgundy/30 last:border-b-0 hover:bg-white/5"
                >
                  <td className="px-3 py-1.5 font-bold text-foreground-dark whitespace-nowrap">
                    {v.key}
                  </td>
                  <td className="px-3 py-1.5">
                    {editingKey === v.key ? (
                      <input
                        type="text"
                        value={editValue}
                        onChange={(e) => setEditValue(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter")
                            handleUpdate(v.key, editValue);
                          if (e.key === "Escape") setEditingKey(null);
                        }}
                        autoFocus
                        className="rounded-none w-full bg-transparent border border-primary-burgundy px-2 py-0.5 text-sm text-foreground-dark outline-none focus:border-accent-orange tabular"
                      />
                    ) : (
                      <span
                        className="cursor-pointer hover:bg-white/5 px-2 py-0.5 inline-block tabular"
                        onClick={() => {
                          setEditingKey(v.key);
                          setEditValue(v.value);
                        }}
                      >
                        {v.value}
                      </span>
                    )}
                  </td>
                  <td className="px-3 py-1.5 text-right">
                    {editingKey === v.key ? (
                      <div className="flex items-center justify-end gap-1">
                        <button
                          onClick={() => handleUpdate(v.key, editValue)}
                          className="rounded-none text-xs font-bold border border-primary-burgundy text-foreground-dark px-2 py-0.5 hover:bg-accent-orange hover:text-background-dark hover:border-accent-orange transition-colors"
                        >
                          [SAVE]
                        </button>
                        <button
                          onClick={() => setEditingKey(null)}
                          className="rounded-none text-xs font-bold border border-primary-burgundy text-foreground-muted px-2 py-0.5 hover:bg-primary-burgundy/10 hover:text-foreground-dark transition-colors"
                        >
                          [ESC]
                        </button>
                      </div>
                    ) : (
                      <div className="flex items-center justify-end gap-1">
                        <button
                          onClick={() => {
                            setEditingKey(v.key);
                            setEditValue(v.value);
                          }}
                          className="rounded-none text-xs font-bold border border-primary-burgundy text-foreground-muted px-2 py-0.5 hover:bg-primary-burgundy/10 hover:text-foreground-dark transition-colors"
                        >
                          [EDIT]
                        </button>
                        <button
                          onClick={() => handleDelete(v.key)}
                          className="rounded-none text-xs font-bold border border-primary-burgundy text-accent-orange px-2 py-0.5 hover:bg-accent-orange hover:text-background-dark hover:border-accent-orange transition-colors"
                        >
                          [DEL]
                        </button>
                      </div>
                    )}
                  </td>
                </tr>
              ))
            )}

            {/* Add row */}
            <tr className="border-t border-primary-burgundy/30 bg-surface-card">
              <td className="px-3 py-1.5">
                <input
                  type="text"
                  placeholder="KEY"
                  value={newKey}
                  onChange={(e) => setNewKey(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && newKey.trim()) handleAdd();
                  }}
                  className="rounded-none w-full bg-transparent border border-primary-burgundy px-2 py-0.5 text-sm text-foreground-dark placeholder:text-foreground-dim outline-none focus:border-accent-orange"
                />
              </td>
              <td className="px-3 py-1.5">
                <input
                  type="text"
                  placeholder="VALUE"
                  value={newValue}
                  onChange={(e) => setNewValue(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && newKey.trim()) handleAdd();
                  }}
                  className="rounded-none w-full bg-transparent border border-primary-burgundy px-2 py-0.5 text-sm text-foreground-dark placeholder:text-foreground-dim outline-none focus:border-accent-orange tabular"
                />
              </td>
              <td className="px-3 py-1.5 text-right">
                <button
                  onClick={handleAdd}
                  disabled={!newKey.trim() || adding}
                  className={`rounded-none text-xs font-bold border px-2 py-0.5 transition-colors ${
                    newKey.trim() && !adding
                      ? "border-primary-burgundy text-foreground-dark hover:bg-accent-orange hover:text-background-dark hover:border-accent-orange"
                      : "border-primary-burgundy/30 text-foreground-dim cursor-not-allowed"
                  }`}
                >
                  [ADD]
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  );
}
