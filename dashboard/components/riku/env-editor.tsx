"use client";

import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";

type Row = { key: string; value: string };

export function EnvEditor({ app }: { app: string }) {
  const [rows, setRows] = useState<Row[]>([]);
  const [original, setOriginal] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);

  const load = () =>
    api.env(app).then((env) => {
      setOriginal(env);
      setRows(Object.entries(env).map(([key, value]) => ({ key, value })));
    });

  useEffect(() => {
    load().catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [app]);

  const setRow = (i: number, patch: Partial<Row>) =>
    setRows((r) => r.map((row, j) => (j === i ? { ...row, ...patch } : row)));

  async function save() {
    setSaving(true);
    try {
      const current = Object.fromEntries(
        rows.filter((r) => r.key.trim()).map((r) => [r.key.trim(), r.value]),
      );
      const set: Record<string, string> = {};
      for (const [k, v] of Object.entries(current)) if (original[k] !== v) set[k] = v;
      const unset = Object.keys(original).filter((k) => !(k in current));
      if (!Object.keys(set).length && !unset.length) {
        toast.info("No changes");
        return;
      }
      await api.setEnv(app, set, unset);
      toast.success("Saved environment: app redeploys");
      setTimeout(load, 800);
    } catch (e) {
      toast.error(`Save failed: ${(e as Error).message}`);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="border border-border bg-card">
      <div className="flex items-center gap-3 border-b border-border px-4 py-2.5">
        <span className="font-mono text-xs tracking-widest text-muted-foreground uppercase">
          environment
        </span>
        <span className="flex-1" />
        <Button size="sm" variant="ghost" onClick={() => setRows((r) => [...r, { key: "", value: "" }])}>
          + add
        </Button>
        <Button size="sm" variant="accent" disabled={saving} onClick={save}>
          {saving ? "saving…" : "save"}
        </Button>
      </div>
      <div className="divide-y divide-border/50">
        {rows.length === 0 && (
          <p className="px-4 py-6 font-mono text-xs text-muted-foreground">
            No variables set. Add one above.
          </p>
        )}
        {rows.map((row, i) => (
          <div key={i} className="flex items-center gap-2 px-3 py-2">
            {/* KEY and VALUE stack full-width (and so equal-width) on mobile,
                go inline at sm+ -- the delete button lives outside this
                group entirely so it stays vertically centered against the
                whole block instead of only against VALUE's row. */}
            <div className="flex flex-1 flex-col gap-1.5 sm:flex-row sm:items-center sm:gap-2">
              <Input
                value={row.key}
                onChange={(e) => setRow(i, { key: e.target.value })}
                placeholder="KEY"
                className="h-7 w-full font-mono text-xs sm:w-64"
              />
              <span className="hidden text-muted-foreground sm:inline">=</span>
              <Input
                value={row.value}
                onChange={(e) => setRow(i, { value: e.target.value })}
                placeholder="value"
                className="h-7 w-full font-mono text-xs sm:flex-1"
              />
            </div>
            <Button
              size="icon-sm"
              variant="destructive"
              className="shrink-0"
              onClick={() => setRows((r) => r.filter((_, j) => j !== i))}
            >
              ✕
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}
