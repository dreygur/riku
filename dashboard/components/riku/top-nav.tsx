"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";
import { api, fmtDur } from "@/lib/api";
import type { RikuState } from "@/lib/types";

const links = [
  { href: "/", label: "overview" },
  { href: "/addons", label: "addons" },
  { href: "/doctor", label: "doctor" },
];

export function TopNav() {
  const path = usePathname();
  const [state, setState] = useState<RikuState | null>(null);
  const [live, setLive] = useState(false);

  useEffect(() => {
    const load = () =>
      api
        .state()
        .then((s) => {
          setState(s);
          setLive(true);
        })
        .catch(() => setLive(false));
    load();
    const t = setInterval(load, 5000);
    return () => clearInterval(t);
  }, []);

  return (
    <header className="sticky top-0 z-20 flex items-center gap-6 border-b border-border bg-background/85 px-5 py-3 backdrop-blur">
      <Link href="/" className="font-mono text-lg font-bold">
        riku<span className="text-[#3fd07f] motion-safe:animate-pulse">▌</span>
      </Link>

      <nav className="flex items-center gap-1">
        {links.map((l) => {
          const active = l.href === "/" ? path === "/" : path.startsWith(l.href);
          return (
            <Link
              key={l.href}
              href={l.href}
              className={cn(
                "px-2.5 py-1 font-mono text-xs tracking-wide uppercase transition-colors",
                active
                  ? "bg-secondary text-foreground"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {l.label}
            </Link>
          );
        })}
      </nav>

      <div className="font-mono text-xs text-muted-foreground">
        {state ? (
          <>
            <span className="text-foreground">v{state.riku_version}</span> · up{" "}
            {fmtDur(state.supervisor_uptime_seconds)} ·{" "}
            <span className="text-foreground">{state.apps.length}</span> app
            {state.apps.length === 1 ? "" : "s"}
          </>
        ) : (
          "connecting…"
        )}
      </div>

      <div className="flex-1" />
      <span
        className={cn(
          "h-1.5 w-1.5",
          live ? "bg-[#3fd07f] motion-safe:animate-pulse" : "bg-muted-foreground/40",
        )}
        title={live ? "live" : "offline"}
      />
    </header>
  );
}
