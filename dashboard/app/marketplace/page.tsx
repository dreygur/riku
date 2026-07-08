"use client";

import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { PageHeader } from "@/components/riku/page-header";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";
import { usePendingRun } from "@/lib/use-pending-run";
import type { MarketplaceSource, MarketplaceHit, TrustKey } from "@/lib/types";

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

const DEFAULT_SOURCE = "github:dreygur/riku";

export default function MarketplacePage() {
  const [sources, setSources] = useState<MarketplaceSource[]>([]);
  const [keys, setKeys] = useState<TrustKey[]>([]);
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<MarketplaceHit[] | null>(null);
  const [url, setUrl] = useState("");
  const [keyName, setKeyName] = useState("");
  const [keyVal, setKeyVal] = useState("");
  const { isPending, run: runPending } = usePendingRun();

  const load = useCallback(() => {
    api.marketSources().then(setSources).catch(() => setSources([]));
    api.trust().then(setKeys).catch(() => setKeys([]));
  }, []);
  useEffect(load, [load]);

  function run(key: string, label: string, fn: () => Promise<void>) {
    runPending(key, label, async () => {
      await fn();
      load();
    });
  }

  const addSource = (value: string) =>
    run("add-source", `Added ${value}`, async () => {
      await api.marketAdd(value);
      setUrl("");
    });

  const search = async () => {
    try {
      setHits(await api.marketSearch(q));
    } catch (e) {
      toast.error(`Search failed: ${(e as Error).message}`);
    }
  };

  return (
    <div className="space-y-6">
      <PageHeader title="marketplace" />

      {/* search + install */}
      <section className="border border-border bg-card">
        <div className="flex flex-wrap items-center gap-2 border-b border-border p-3">
          <Input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && search()}
            placeholder="search plugins across sources…"
            className="h-7 min-w-40 flex-1 font-mono text-xs"
          />
          <Button size="sm" variant="secondary" onClick={search}>
            search
          </Button>
        </div>
        {hits === null ? (
          <p className="px-4 py-4 font-mono text-xs text-muted-foreground">
            search to browse, or install directly from the sources below.
          </p>
        ) : hits.length === 0 ? (
          <p className="px-4 py-4 font-mono text-xs text-muted-foreground">no matches.</p>
        ) : (
          <div className="divide-y divide-border/50">
            {hits.map((h) => {
              const installKey = `install-${h.marketplace}/${h.name}`;
              return (
                <div
                  key={`${h.marketplace}/${h.name}`}
                  className="flex flex-wrap items-center gap-x-3 gap-y-1 px-4 py-3 font-mono text-xs"
                >
                  <span className="font-bold">{h.name}</span>
                  <span className="text-muted-foreground">@{h.marketplace}</span>
                  <span className="min-w-0 break-words text-muted-foreground">{h.description}</span>
                  <span className="flex-1" />
                  <Button
                    size="sm"
                    variant="accent"
                    disabled={isPending(installKey)}
                    onClick={() =>
                      run(installKey, `Installed ${h.name}`, () => api.pluginInstall(h.source))
                    }
                  >
                    {isPending(installKey) ? "installing…" : "install"}
                  </Button>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {/* sources */}
      <section className="border border-border bg-card">
        <div className="flex flex-col gap-2 border-b border-border p-3 sm:flex-row sm:items-end">
          <span className="font-mono text-xs tracking-widest text-muted-foreground uppercase sm:self-center">
            sources
          </span>
          <span className="hidden flex-1 sm:block" />
          <div className="flex flex-col gap-2 sm:flex-row sm:items-end">
            <Field
              label="source"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="github:owner/repo"
              className="h-7 w-full font-mono text-xs sm:w-56"
            />
            <Button
              size="sm"
              variant="secondary"
              className="shrink-0"
              disabled={!url || isPending("add-source")}
              onClick={() => addSource(url)}
            >
              {isPending("add-source") ? "adding…" : "add source"}
            </Button>
          </div>
        </div>
        {sources.length === 0 ? (
          <p className="px-4 py-4 font-mono text-xs text-muted-foreground">
            No sources. The repo itself is a marketplace:{" "}
            <button
              type="button"
              className="text-primary underline underline-offset-2 hover:text-primary/80"
              disabled={isPending("add-source")}
              onClick={() => addSource(DEFAULT_SOURCE)}
            >
              add {DEFAULT_SOURCE}
            </button>
            .
          </p>
        ) : (
          <div className="divide-y divide-border/50">
            {sources.map((s) => {
              const removeKey = `remove-source-${s.name}`;
              return (
                <div
                  key={s.name}
                  className="flex flex-wrap items-center gap-x-3 gap-y-1 px-4 py-3 font-mono text-xs"
                >
                  <span className="font-bold">{s.name}</span>
                  <span className="min-w-0 break-words text-muted-foreground">{s.url}</span>
                  <span className="flex-1" />
                  <Button
                    size="sm"
                    variant="destructive"
                    disabled={isPending(removeKey)}
                    onClick={() =>
                      run(removeKey, `Removed ${s.name}`, () => api.marketRemove(s.name))
                    }
                  >
                    {isPending(removeKey) ? "removing…" : "remove"}
                  </Button>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {/* trust keyring */}
      <section className="border border-border bg-card">
        <div className="flex flex-col gap-2 border-b border-border p-3 sm:flex-row sm:items-end">
          <span className="font-mono text-xs tracking-widest text-muted-foreground uppercase sm:self-center">
            trust keyring
          </span>
          <span className="hidden flex-1 sm:block" />
          <div className="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-end">
            <Field
              label="author"
              value={keyName}
              onChange={(e) => setKeyName(e.target.value)}
              placeholder="name"
              className="h-7 w-full font-mono text-xs sm:w-28"
            />
            <Field
              label="ed25519 pubkey"
              value={keyVal}
              onChange={(e) => setKeyVal(e.target.value)}
              placeholder="hex"
              className="h-7 w-full font-mono text-xs sm:w-56"
            />
            <Button
              size="sm"
              variant="accent"
              className="shrink-0"
              disabled={!keyName || !keyVal || isPending("trust")}
              onClick={() =>
                run("trust", `Trusted ${keyName}`, async () => {
                  await api.trustAdd(keyName, keyVal);
                  setKeyName("");
                  setKeyVal("");
                })
              }
            >
              {isPending("trust") ? "trusting…" : "trust"}
            </Button>
          </div>
        </div>
        {keys.length === 0 ? (
          <p className="px-4 py-4 font-mono text-xs text-muted-foreground">
            No trusted keys. Signed bundles install only if a trusted key verifies them.
          </p>
        ) : (
          <div className="divide-y divide-border/50">
            {keys.map((k) => {
              const untrustKey = `untrust-${k.name}`;
              return (
                <div key={k.name} className="flex items-center gap-3 px-4 py-3 font-mono text-xs">
                  <span className="font-bold">{k.name}</span>
                  <span className="truncate text-muted-foreground">{k.pubkey}</span>
                  <span className="flex-1" />
                  <Button
                    size="sm"
                    variant="destructive"
                    disabled={isPending(untrustKey)}
                    onClick={() =>
                      run(untrustKey, `Untrusted ${k.name}`, () => api.trustRemove(k.name))
                    }
                  >
                    {isPending(untrustKey) ? "removing…" : "remove"}
                  </Button>
                </div>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}
