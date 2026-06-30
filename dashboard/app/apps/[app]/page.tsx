"use client";

import { use, useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import { AppCard } from "@/components/riku/app-card";
import { EnvEditor } from "@/components/riku/env-editor";
import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import type { AppState } from "@/lib/types";

export default function AppDetail({ params }: { params: Promise<{ app: string }> }) {
  const { app } = use(params);
  const [state, setState] = useState<AppState | null>(null);
  const [missing, setMissing] = useState(false);

  const load = useCallback(async () => {
    try {
      const s = await api.state();
      const found = s.apps.find((a) => a.app === app);
      if (found) setState(found);
      else setMissing(true);
    } catch {
      /* nav surfaces connection */
    }
  }, [app]);

  useEffect(() => {
    load();
    const t = setInterval(load, 4000);
    return () => clearInterval(t);
  }, [load]);

  if (missing) {
    return (
      <div className="py-20 text-center text-muted-foreground">
        App <code className="font-mono text-foreground">{app}</code> not found.{" "}
        <Link href="/" className="text-[#3fd07f] underline">
          back to overview
        </Link>
      </div>
    );
  }

  if (!state) {
    return <p className="py-20 text-center font-mono text-sm text-muted-foreground">loading…</p>;
  }

  return (
    <div className="space-y-5">
      <div className="flex items-center gap-3">
        <Link href="/" className="font-mono text-xs text-muted-foreground hover:text-foreground">
          ‹ overview
        </Link>
        <h1 className="font-mono text-xl font-bold">{state.app}</h1>
        <span className="flex-1" />
        <Button
          size="sm"
          variant="secondary"
          onClick={() =>
            api
              .backup(state.app)
              .then(() => toast.success(`Backed up ${state.app}`))
              .catch((e) => toast.error(`Backup failed: ${e.message}`))
          }
        >
          back up app
        </Button>
      </div>

      <AppCard app={state} onChanged={load} />
      <EnvEditor app={state.app} />
    </div>
  );
}
