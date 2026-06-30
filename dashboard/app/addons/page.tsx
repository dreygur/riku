"use client";

import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";
import type { AddonInstance } from "@/lib/types";

export default function AddonsPage() {
  const [instances, setInstances] = useState<AddonInstance[] | null>(null);
  const [plugin, setPlugin] = useState("");
  const [name, setName] = useState("");
  const [bindApp, setBindApp] = useState<Record<string, string>>({});

  const load = useCallback(() => {
    api.addons().then(setInstances).catch(() => setInstances([]));
  }, []);
  useEffect(load, [load]);

  async function run(label: string, fn: () => Promise<void>) {
    try {
      await fn();
      toast.success(label);
      load();
    } catch (e) {
      toast.error(`${label} failed: ${(e as Error).message}`);
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="mb-3 font-mono text-sm tracking-widest text-muted-foreground uppercase">
          managed datastores
        </h1>
        {/* create */}
        <div className="flex flex-wrap items-center gap-2 border border-border bg-card p-3">
          <Input
            value={plugin}
            onChange={(e) => setPlugin(e.target.value)}
            placeholder="addon plugin (e.g. postgres)"
            className="h-8 w-56 font-mono text-xs"
          />
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="instance name"
            className="h-8 w-48 font-mono text-xs"
          />
          <Button
            size="sm"
            variant="secondary"
            className="border-[#3fd07f]/30"
            disabled={!plugin || !name}
            onClick={() =>
              run(`Provisioned ${name}`, async () => {
                await api.addonCreate(plugin, name);
                setPlugin("");
                setName("");
              })
            }
          >
            provision
          </Button>
        </div>
      </div>

      {/* instances */}
      {!instances ? (
        <p className="font-mono text-xs text-muted-foreground">loading…</p>
      ) : instances.length === 0 ? (
        <div className="border border-border bg-card px-4 py-10 text-center text-muted-foreground">
          No addon instances yet. Provision one above (needs the addon plugin installed).
        </div>
      ) : (
        instances.map((inst) => {
          const apps = Object.keys(inst.bindings ?? {});
          return (
            <section key={inst.instance} className="border border-border bg-card">
              <div className="flex flex-wrap items-center gap-3 border-b border-border px-4 py-3">
                <span className="font-mono text-sm font-bold">{inst.instance}</span>
                <span className="border border-info/30 px-2 py-0.5 font-mono text-[11px] text-info">
                  {inst.plugin}
                </span>
                <span className="font-mono text-[11px] text-muted-foreground">
                  {apps.length ? `bound: ${apps.join(", ")}` : "unbound"}
                </span>
                <span className="flex-1" />
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() => run(`Backed up ${inst.instance}`, () => api.addonBackup(inst.instance))}
                >
                  backup
                </Button>
                <Button
                  size="sm"
                  variant="secondary"
                  className="hover:border-destructive/50 hover:text-destructive"
                  onClick={() => {
                    if (confirm(`Destroy ${inst.instance}? Its data is removed.`))
                      run(`Destroyed ${inst.instance}`, () => api.addonDestroy(inst.instance));
                  }}
                >
                  destroy
                </Button>
              </div>
              <div className="flex flex-wrap items-center gap-2 px-4 py-3">
                <Input
                  value={bindApp[inst.instance] ?? ""}
                  onChange={(e) =>
                    setBindApp((m) => ({ ...m, [inst.instance]: e.target.value }))
                  }
                  placeholder="app name"
                  className="h-8 w-48 font-mono text-xs"
                />
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() =>
                    run(`Bound ${inst.instance}`, () =>
                      api.addonBind(inst.instance, bindApp[inst.instance] ?? ""),
                    )
                  }
                >
                  bind
                </Button>
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() =>
                    run(`Unbound ${inst.instance}`, () =>
                      api.addonUnbind(inst.instance, bindApp[inst.instance] ?? ""),
                    )
                  }
                >
                  unbind
                </Button>
              </div>
            </section>
          );
        })
      )}
    </div>
  );
}
