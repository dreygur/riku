"use client";

import { useCallback, useEffect, useState } from "react";
import { PageHeader } from "@/components/riku/page-header";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { confirmDialog } from "@/components/riku/confirm-dialog";
import { api } from "@/lib/api";
import { usePendingRun } from "@/lib/use-pending-run";
import type { AddonInstance } from "@/lib/types";

function Field({
  label,
  ...props
}: React.ComponentProps<typeof Input> & { label: string }) {
  return (
    <span className="flex w-full flex-col gap-1 sm:w-auto">
      <span className="text-[10px] tracking-wider text-muted-foreground uppercase">
        {label}
      </span>
      <Input {...props} />
    </span>
  );
}

export default function AddonsPage() {
  const [instances, setInstances] = useState<AddonInstance[] | null>(null);
  const [plugin, setPlugin] = useState("");
  const [name, setName] = useState("");
  const [bindApp, setBindApp] = useState<Record<string, string>>({});
  const { isPending, run: runPending } = usePendingRun();

  const load = useCallback(() => {
    api.addons().then(setInstances).catch(() => setInstances([]));
  }, []);
  useEffect(load, [load]);

  function run(key: string, label: string, fn: () => Promise<void>) {
    runPending(key, label, async () => {
      await fn();
      load();
    });
  }

  return (
    <div className="space-y-6">
      <div>
        <div className="mb-3">
          <PageHeader title="managed datastores" />
        </div>
        {/* create */}
        <div className="flex flex-col gap-2 border border-border bg-card p-3 sm:flex-row sm:flex-wrap sm:items-end">
          <Field
            label="addon plugin"
            value={plugin}
            onChange={(e) => setPlugin(e.target.value)}
            placeholder="e.g. postgres"
            className="h-7 w-full font-mono text-xs sm:w-40"
          />
          <Field
            label="instance name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. db1"
            className="h-7 w-full font-mono text-xs sm:w-40"
          />
          <Button
            size="sm"
            variant="accent"
            disabled={!plugin || !name || isPending("provision")}
            onClick={() =>
              run("provision", `Provisioned ${name}`, async () => {
                await api.addonCreate(plugin, name);
                setPlugin("");
                setName("");
              })
            }
          >
            {isPending("provision") ? "provisioning…" : "provision"}
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
          const backupKey = `backup-${inst.instance}`;
          const destroyKey = `destroy-${inst.instance}`;
          const bindKey = `bind-${inst.instance}`;
          const unbindKey = `unbind-${inst.instance}`;
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
                  disabled={isPending(backupKey)}
                  onClick={() =>
                    run(backupKey, `Backed up ${inst.instance}`, () =>
                      api.addonBackup(inst.instance),
                    )
                  }
                >
                  {isPending(backupKey) ? "backing up…" : "backup"}
                </Button>
                <Button
                  size="sm"
                  variant="destructive"
                  disabled={isPending(destroyKey)}
                  onClick={async () => {
                    const ok = await confirmDialog(
                      `Destroy ${inst.instance}? Its data is removed.`,
                    );
                    if (ok)
                      run(destroyKey, `Destroyed ${inst.instance}`, () =>
                        api.addonDestroy(inst.instance),
                      );
                  }}
                >
                  {isPending(destroyKey) ? "destroying…" : "destroy"}
                </Button>
              </div>
              <div className="flex flex-col gap-2 px-4 py-3 sm:flex-row sm:flex-wrap sm:items-end">
                <Field
                  label="app name"
                  value={bindApp[inst.instance] ?? ""}
                  onChange={(e) =>
                    setBindApp((m) => ({ ...m, [inst.instance]: e.target.value }))
                  }
                  placeholder="myapp"
                  className="h-7 w-full font-mono text-xs sm:w-40"
                />
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={isPending(bindKey)}
                  onClick={() =>
                    run(bindKey, `Bound ${inst.instance}`, () =>
                      api.addonBind(inst.instance, bindApp[inst.instance] ?? ""),
                    )
                  }
                >
                  {isPending(bindKey) ? "binding…" : "bind"}
                </Button>
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={isPending(unbindKey)}
                  onClick={() =>
                    run(unbindKey, `Unbound ${inst.instance}`, () =>
                      api.addonUnbind(inst.instance, bindApp[inst.instance] ?? ""),
                    )
                  }
                >
                  {isPending(unbindKey) ? "unbinding…" : "unbind"}
                </Button>
              </div>
            </section>
          );
        })
      )}
    </div>
  );
}
