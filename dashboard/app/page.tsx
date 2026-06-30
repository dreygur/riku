"use client";

import { useCallback, useEffect, useState } from "react";
import { AppCard } from "@/components/riku/app-card";
import { Reveal } from "@/components/riku/reveal";
import { api } from "@/lib/api";
import type { RikuState } from "@/lib/types";

export default function Overview() {
  const [state, setState] = useState<RikuState | null>(null);

  const load = useCallback(async () => {
    try {
      setState(await api.state());
    } catch {
      /* TopNav surfaces connection state */
    }
  }, []);

  useEffect(() => {
    load();
    const t = setInterval(load, 4000);
    return () => clearInterval(t);
  }, [load]);

  if (!state) {
    return (
      <p className="py-20 text-center font-mono text-sm text-muted-foreground">
        reading the supervisor…
      </p>
    );
  }

  if (state.apps.length === 0) {
    return (
      <div className="py-20 text-center text-muted-foreground">
        No apps deployed yet.
        <br />
        <br />
        Push one:{" "}
        <code className="bg-card px-2 py-1 font-mono text-[#3fd07f]">git push riku main</code>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {state.apps.map((a, i) => (
        <Reveal key={a.app} i={i}>
          <AppCard app={a} onChanged={load} />
        </Reveal>
      ))}
    </div>
  );
}
