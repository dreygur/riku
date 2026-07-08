"use client";

import Link from "next/link";
import { ChevronLeftIcon } from "lucide-react";
import type { ReactNode } from "react";

export function PageHeader({
  title,
  variant = "label",
  meta,
  actions,
}: {
  title: string;
  variant?: "label" | "title";
  meta?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
      <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:gap-3">
        <Link
          href="/"
          className="inline-flex items-center gap-0.5 self-start font-sans text-xs text-muted-foreground transition-colors hover:text-foreground"
        >
          <ChevronLeftIcon className="size-3.5" />
          overview
        </Link>
        <h1
          className={
            variant === "title"
              ? "font-mono text-xl font-bold"
              : "font-sans text-sm font-medium tracking-widest text-muted-foreground uppercase"
          }
        >
          {title}
        </h1>
        {meta}
      </div>
      {actions && (
        <>
          <span className="hidden flex-1 sm:block" />
          <div className="flex flex-wrap gap-2">{actions}</div>
        </>
      )}
    </div>
  );
}
