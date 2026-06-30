"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { PluginsList } from "@/lib/types";

function Section({
  title,
  count,
  children,
}: {
  title: string;
  count: number;
  children: React.ReactNode;
}) {
  return (
    <section className="border border-border bg-card">
      <div className="flex items-center gap-3 border-b border-border px-4 py-2.5">
        <span className="font-mono text-xs tracking-widest text-muted-foreground uppercase">
          {title}
        </span>
        <span className="font-mono text-[11px] text-muted-foreground">{count}</span>
      </div>
      {count === 0 ? (
        <p className="px-4 py-4 font-mono text-xs text-muted-foreground">none installed</p>
      ) : (
        children
      )}
    </section>
  );
}

export default function PluginsPage() {
  const [data, setData] = useState<PluginsList | null>(null);

  useEffect(() => {
    api.plugins().then(setData).catch(() => setData({ runtimes: [], hooks: [], bundles: [] }));
  }, []);

  if (!data) {
    return <p className="py-20 text-center font-mono text-sm text-muted-foreground">loading…</p>;
  }

  return (
    <div className="space-y-5">
      <h1 className="font-mono text-sm tracking-widest text-muted-foreground uppercase">
        plugins
      </h1>

      <Section title="runtimes" count={data.runtimes.length}>
        <div className="flex flex-wrap gap-2 px-4 py-3">
          {data.runtimes.map((r) => (
            <span key={r} className="border border-border px-2.5 py-1 font-mono text-xs">
              {r}
            </span>
          ))}
        </div>
      </Section>

      <Section title="lifecycle hooks" count={data.hooks.length}>
        <div className="flex flex-wrap gap-2 px-4 py-3">
          {data.hooks.map((h) => (
            <span key={h} className="border border-border px-2.5 py-1 font-mono text-xs">
              {h}
            </span>
          ))}
        </div>
      </Section>

      <Section title="bundles" count={data.bundles.length}>
        <div className="divide-y divide-border/50">
          {data.bundles.map((b) => (
            <div key={b.name} className="flex items-center gap-3 px-4 py-3 font-mono text-xs">
              <span className="font-bold">{b.name}</span>
              <span className="text-muted-foreground">{b.version}</span>
              <span className="border border-info/30 px-2 py-0.5 text-[11px] text-info">
                {b.type}
              </span>
              <span className="text-muted-foreground">{b.description}</span>
            </div>
          ))}
        </div>
      </Section>
    </div>
  );
}
