"use client";

import { use, useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import { AppCard } from "@/components/riku/app-card";
import { EnvEditor } from "@/components/riku/env-editor";
import { PageHeader } from "@/components/riku/page-header";
import { Button } from "@/components/ui/button";
import { confirmDialog } from "@/components/riku/confirm-dialog";
import { api } from "@/lib/api";
import { usePendingRun } from "@/lib/use-pending-run";
import type { AppState } from "@/lib/types";

export default function AppDetail({ params }: { params: Promise<{ app: string }> }) {
  const { app } = use(params);
  const [state, setState] = useState<AppState | null>(null);
  const [missing, setMissing] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const restoreInput = useRef<HTMLInputElement>(null);
  const { isPending, run } = usePendingRun();

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
        <Link href="/" className="text-primary underline">
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
      <PageHeader
        title={state.app}
        variant="title"
        actions={
          <>
            <Button
              size="sm"
              variant="secondary"
              disabled={isPending("backup")}
              onClick={() => run("backup", `Backed up ${state.app}`, () => api.backup(state.app))}
            >
              {isPending("backup") ? "backing up…" : "back up app"}
            </Button>
            <Button
              size="sm"
              variant="secondary"
              disabled={restoring}
              onClick={() => restoreInput.current?.click()}
            >
              {restoring ? "restoring…" : "restore from file"}
            </Button>
            <input
              ref={restoreInput}
              type="file"
              accept=".gz,.tar.gz,application/gzip"
              className="hidden"
              onChange={async (e) => {
                const file = e.target.files?.[0];
                e.target.value = "";
                if (!file) return;
                const ok = await confirmDialog(
                  `Restore ${state.app} from "${file.name}"? This overwrites the app's current source, env, and data.`,
                );
                if (!ok) return;
                setRestoring(true);
                api
                  .restore(state.app, file)
                  .then(() => {
                    toast.success(`Restored ${state.app}: redeploy or restart to bring it up`);
                    load();
                  })
                  .catch((err) => toast.error(`Restore failed: ${err.message}`))
                  .finally(() => setRestoring(false));
              }}
            />
          </>
        }
      />

      <AppCard app={state} onChanged={load} />
      <EnvEditor app={state.app} />
    </div>
  );
}
