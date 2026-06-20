"use client";

import * as React from "react";

export interface CollapsibleSectionProps {
  title: string;
  storageKey: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
  className?: string;
}

// Low-churn config panels (plugins, nginx routes, ...) sit behind a
// disclosure toggle instead of always-rendered: they rarely change between
// glances at the dashboard, so showing them by default just pushes the
// worker grid and logs further down the page. Remembered per-browser via
// localStorage so toggling it open isn't a tax paid on every reload.
export function CollapsibleSection({
  title,
  storageKey,
  defaultOpen = false,
  children,
  className,
}: CollapsibleSectionProps) {
  const [open, setOpen] = React.useState(defaultOpen);
  const fullKey = `riku-dashboard-collapsible:${storageKey}`;

  React.useEffect(() => {
    const stored = window.localStorage.getItem(fullKey);
    if (stored !== null) setOpen(stored === "1");
  }, [fullKey]);

  const toggle = React.useCallback(() => {
    setOpen((prev) => {
      const next = !prev;
      window.localStorage.setItem(fullKey, next ? "1" : "0");
      return next;
    });
  }, [fullKey]);

  return (
    <div className={`border-b border-line ${className ?? ""}`}>
      <button
        type="button"
        onClick={toggle}
        data-testid={`collapsible-toggle-${storageKey}`}
        aria-expanded={open}
        className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-white/5"
      >
        <span className="font-display text-xs font-bold tracking-wide text-accent-amber tabular">
          [{open ? "-" : "+"}]
        </span>
        <span className="font-display text-xs font-bold tracking-wide text-foreground-muted">
          {title}
        </span>
      </button>
      {open && <div data-testid={`collapsible-body-${storageKey}`}>{children}</div>}
    </div>
  );
}
