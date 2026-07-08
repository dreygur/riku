"use client";

import { use, useEffect, useState } from "react";
import Link from "next/link";
import { PageHeader } from "@/components/riku/page-header";
import { api } from "@/lib/api";
import type { UiPanel } from "@/lib/types";

export default function PluginUiPage({ params }: { params: Promise<{ name: string }> }) {
  const { name } = use(params);
  const [panel, setPanel] = useState<UiPanel | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setPanel(null);
    setFailed(false);
    api
      .pluginUi(name)
      .then(setPanel)
      .catch(() => setFailed(true));
  }, [name]);

  if (failed) {
    return (
      <div className="py-20 text-center text-muted-foreground">
        Plugin <code className="font-mono text-foreground">{name}</code> has no UI panel.{" "}
        <Link href="/plugins" className="text-primary underline">
          back to plugins
        </Link>
      </div>
    );
  }

  if (!panel) {
    return <p className="py-20 text-center font-mono text-sm text-muted-foreground">loading…</p>;
  }

  return (
    <div className="space-y-5">
      <PageHeader title={name} />

      {panel.sections.length === 0 ? (
        <p className="py-10 text-center font-mono text-xs text-muted-foreground">
          this plugin returned an empty panel
        </p>
      ) : (
        panel.sections.map((section, i) => (
          <section key={i} className="border border-border bg-card">
            <div className="border-b border-border px-4 py-2.5">
              <span className="font-mono text-xs tracking-widest text-muted-foreground uppercase">
                {section.title}
              </span>
            </div>
            <div className="divide-y divide-border/50">
              {section.fields.map((field, j) => (
                <div key={j} className="flex items-center justify-between gap-3 px-4 py-2.5">
                  <span className="font-mono text-xs text-muted-foreground">{field.label}</span>
                  <span className="font-mono text-xs">{field.value}</span>
                </div>
              ))}
            </div>
          </section>
        ))
      )}
    </div>
  );
}
