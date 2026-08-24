"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import { LockIcon, MenuIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { api, fmtDur } from "@/lib/api";
import type { RikuState } from "@/lib/types";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";

const links = [
  { href: "/", label: "overview" },
  { href: "/metrics", label: "metrics" },
  { href: "/plugins", label: "plugins" },
  { href: "/marketplace", label: "market" },
  { href: "/addons", label: "addons" },
  { href: "/doctor", label: "doctor" },
];

function StatusText({ state }: { state: RikuState | null }) {
  if (!state) return <>connecting…</>;
  return (
    <>
      <span className="text-foreground">v{state.riku_version}</span> · up{" "}
      {fmtDur(state.supervisor_uptime_seconds)} ·{" "}
      <span className="text-foreground">{state.apps.length}</span> app
      {state.apps.length === 1 ? "" : "s"}
    </>
  );
}

export function TopNav() {
  const path = usePathname();
  const [state, setState] = useState<RikuState | null>(null);
  const [live, setLive] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const [pluginLinks, setPluginLinks] = useState<{ href: string; label: string }[]>([]);

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

  // Plugin-declared nav entries change only on install/remove, not worth
  // polling every 5s alongside supervisor state: fetch once on mount.
  useEffect(() => {
    api
      .plugins()
      .then((data) => {
        setPluginLinks(
          data.bundles
            .filter((b) => b.ui?.nav_label)
            .map((b) => ({ href: `/plugins/${b.name}`, label: b.ui!.nav_label })),
        );
      })
      .catch(() => setPluginLinks([]));
  }, []);

  const allLinks = [...links, ...pluginLinks];

  const navLink = (l: { href: string; label: string }, onClick?: () => void) => {
    const active = l.href === "/" ? path === "/" : path.startsWith(l.href);
    return (
      <Link
        key={l.href}
        href={l.href}
        onClick={onClick}
        className={cn(
          "px-2.5 py-1 font-sans text-xs font-medium tracking-wide uppercase transition-colors",
          active ? "bg-secondary text-foreground" : "text-muted-foreground hover:text-foreground",
        )}
      >
        {l.label}
      </Link>
    );
  };

  if (path === "/login") return null;

  return (
    <header className="sticky top-0 z-20 flex items-center gap-3 border-b border-border bg-background/85 px-3 py-3 backdrop-blur sm:gap-6 sm:px-5">
      <button
        className="text-muted-foreground hover:text-foreground md:hidden"
        onClick={() => setMobileOpen(true)}
        aria-label="open menu"
      >
        <MenuIcon className="size-5" />
      </button>

      <Link href="/" className="font-mono text-lg font-bold">
        riku<span className="text-primary motion-safe:animate-pulse">▌</span>
      </Link>

      <nav className="hidden items-center gap-1 md:flex">
        {allLinks.map((l) => navLink(l))}
      </nav>

      <div className="hidden font-mono text-xs text-muted-foreground md:block">
        <StatusText state={state} />
      </div>

      <div className="flex-1" />
      <button
        onClick={() => window.dispatchEvent(new Event("riku-open-command"))}
        className="flex h-7 shrink-0 items-center gap-0.5 border border-border px-2.5 font-mono text-[11px] text-muted-foreground hover:text-foreground"
        title="command palette"
      >
        {/* The ⌘ glyph renders noticeably smaller than a letter at the same
            font-size in most monospace fonts -- bump it up to visually
            balance against K instead of looking like an afterthought. */}
        <span className="text-sm leading-none">⌘</span>
        <span>K</span>
      </button>
      <span
        className={cn(
          "h-1.5 w-1.5 shrink-0",
          live ? "bg-primary motion-safe:animate-pulse" : "bg-muted-foreground/40",
        )}
        title={live ? "live" : "offline"}
      />
      <button
        onClick={async () => {
          await fetch("/api/logout", { method: "POST" });
          window.location.href = "/login";
        }}
        className="text-muted-foreground hover:text-foreground"
        title="lock dashboard"
        aria-label="lock dashboard"
      >
        <LockIcon className="size-4" />
      </button>

      <Sheet open={mobileOpen} onOpenChange={setMobileOpen}>
        <SheetContent side="left" className="w-64 border-r border-border bg-background p-0">
          <SheetHeader className="border-b border-border px-4 py-3">
            <SheetTitle className="font-mono text-base font-bold">
              riku<span className="text-primary">▌</span>
            </SheetTitle>
          </SheetHeader>
          <nav className="flex flex-col gap-1 p-2">
            {allLinks.map((l) => navLink(l, () => setMobileOpen(false)))}
          </nav>
          <div className="border-t border-border px-4 py-3 font-mono text-xs text-muted-foreground">
            <StatusText state={state} />
          </div>
        </SheetContent>
      </Sheet>
    </header>
  );
}
