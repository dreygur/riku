"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";
import { api } from "@/lib/api";
import type { AppState } from "@/lib/types";

export function CommandMenu() {
  const [open, setOpen] = useState(false);
  const [apps, setApps] = useState<AppState[]>([]);
  const router = useRouter();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((o) => !o);
      }
    };
    const onOpen = () => setOpen(true);
    window.addEventListener("keydown", onKey);
    window.addEventListener("riku-open-command", onOpen);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("riku-open-command", onOpen);
    };
  }, []);

  useEffect(() => {
    if (open) api.state().then((s) => setApps(s.apps)).catch(() => {});
  }, [open]);

  const go = (fn: () => void) => {
    setOpen(false);
    fn();
  };

  const act = (label: string, fn: () => Promise<void>) =>
    go(() =>
      fn()
        .then(() => toast.success(label))
        .catch((e) => toast.error(`${label} failed: ${(e as Error).message}`)),
    );

  return (
    <CommandDialog open={open} onOpenChange={setOpen} className="font-mono">
      <CommandInput placeholder="run a command…  (apps, actions, pages)" />
      <CommandList>
        <CommandEmpty>No match.</CommandEmpty>

        <CommandGroup heading="go to">
          <CommandItem onSelect={() => go(() => router.push("/"))}>Overview</CommandItem>
          <CommandItem onSelect={() => go(() => router.push("/addons"))}>Addons</CommandItem>
          <CommandItem onSelect={() => go(() => router.push("/doctor"))}>Doctor</CommandItem>
        </CommandGroup>

        {apps.map((a) => (
          <div key={a.app}>
            <CommandSeparator />
            <CommandGroup heading={a.app}>
              <CommandItem onSelect={() => go(() => router.push(`/apps/${a.app}`))}>
                Open {a.app}
              </CommandItem>
              <CommandItem onSelect={() => act(`Redeployed ${a.app}`, () => api.redeploy(a.app))}>
                Redeploy {a.app}
              </CommandItem>
              <CommandItem onSelect={() => act(`Restarted ${a.app}`, () => api.restart(a.app))}>
                Restart {a.app}
              </CommandItem>
              <CommandItem onSelect={() => act(`Stopped ${a.app}`, () => api.stop(a.app))}>
                Stop {a.app}
              </CommandItem>
              <CommandItem onSelect={() => act(`Rolled back ${a.app}`, () => api.rollback(a.app))}>
                Roll back {a.app}
              </CommandItem>
            </CommandGroup>
          </div>
        ))}
      </CommandList>
    </CommandDialog>
  );
}
