"use client";

import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";
import type { MarketplaceSource, MarketplaceHit, TrustKey } from "@/lib/types";

export default function MarketplacePage() {
  const [sources, setSources] = useState<MarketplaceSource[]>([]);
  const [keys, setKeys] = useState<TrustKey[]>([]);
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<MarketplaceHit[] | null>(null);
  const [url, setUrl] = useState("");
  const [keyName, setKeyName] = useState("");
  const [keyVal, setKeyVal] = useState("");

  const load = useCallback(() => {
    api.marketSources().then(setSources).catch(() => setSources([]));
    api.trust().then(setKeys).catch(() => setKeys([]));
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

  const search = async () => {
    try {
      setHits(await api.marketSearch(q));
    } catch (e) {
      toast.error(`Search failed: ${(e as Error).message}`);
    }
  };

  return (
    <div className="space-y-6">
      <h1 className="font-mono text-sm tracking-widest text-muted-foreground uppercase">
        marketplace
      </h1>

      {/* search + install */}
      <section className="border border-border bg-card">
        <div className="flex items-center gap-2 border-b border-border p-3">
          <Input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && search()}
            placeholder="search plugins across sources…"
            className="h-8 flex-1 font-mono text-xs"
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
            {hits.map((h) => (
              <div key={`${h.marketplace}/${h.name}`} className="flex items-center gap-3 px-4 py-3 font-mono text-xs">
                <span className="font-bold">{h.name}</span>
                <span className="text-muted-foreground">@{h.marketplace}</span>
                <span className="text-muted-foreground">{h.description}</span>
                <span className="flex-1" />
                <Button
                  size="sm"
                  variant="secondary"
                  className="border-[#3fd07f]/30"
                  onClick={() => run(`Installed ${h.name}`, () => api.pluginInstall(h.source))}
                >
                  install
                </Button>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* sources */}
      <section className="border border-border bg-card">
        <div className="flex items-center gap-2 border-b border-border p-3">
          <span className="font-mono text-xs tracking-widest text-muted-foreground uppercase">
            sources
          </span>
          <span className="flex-1" />
          <Input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="github:owner/repo"
            className="h-8 w-64 font-mono text-xs"
          />
          <Button
            size="sm"
            variant="secondary"
            disabled={!url}
            onClick={() => run(`Added source`, async () => { await api.marketAdd(url); setUrl(""); })}
          >
            add source
          </Button>
        </div>
        {sources.length === 0 ? (
          <p className="px-4 py-4 font-mono text-xs text-muted-foreground">
            No sources. The repo itself is a marketplace: add{" "}
            <code className="text-[#3fd07f]">github:dreygur/riku</code>.
          </p>
        ) : (
          <div className="divide-y divide-border/50">
            {sources.map((s) => (
              <div key={s.name} className="flex items-center gap-3 px-4 py-3 font-mono text-xs">
                <span className="font-bold">{s.name}</span>
                <span className="text-muted-foreground">{s.url}</span>
                <span className="flex-1" />
                <Button
                  size="sm"
                  variant="ghost"
                  className="text-muted-foreground hover:text-destructive"
                  onClick={() => run(`Removed ${s.name}`, () => api.marketRemove(s.name))}
                >
                  remove
                </Button>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* trust keyring */}
      <section className="border border-border bg-card">
        <div className="flex flex-wrap items-center gap-2 border-b border-border p-3">
          <span className="font-mono text-xs tracking-widest text-muted-foreground uppercase">
            trust keyring
          </span>
          <span className="flex-1" />
          <Input
            value={keyName}
            onChange={(e) => setKeyName(e.target.value)}
            placeholder="author"
            className="h-8 w-32 font-mono text-xs"
          />
          <Input
            value={keyVal}
            onChange={(e) => setKeyVal(e.target.value)}
            placeholder="ed25519 pubkey (hex)"
            className="h-8 w-72 font-mono text-xs"
          />
          <Button
            size="sm"
            variant="secondary"
            disabled={!keyName || !keyVal}
            onClick={() =>
              run(`Trusted ${keyName}`, async () => {
                await api.trustAdd(keyName, keyVal);
                setKeyName("");
                setKeyVal("");
              })
            }
          >
            trust
          </Button>
        </div>
        {keys.length === 0 ? (
          <p className="px-4 py-4 font-mono text-xs text-muted-foreground">
            No trusted keys. Signed bundles install only if a trusted key verifies them.
          </p>
        ) : (
          <div className="divide-y divide-border/50">
            {keys.map((k) => (
              <div key={k.name} className="flex items-center gap-3 px-4 py-3 font-mono text-xs">
                <span className="font-bold">{k.name}</span>
                <span className="truncate text-muted-foreground">{k.pubkey}</span>
                <span className="flex-1" />
                <Button
                  size="sm"
                  variant="ghost"
                  className="text-muted-foreground hover:text-destructive"
                  onClick={() => run(`Untrusted ${k.name}`, () => api.trustRemove(k.name))}
                >
                  remove
                </Button>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
