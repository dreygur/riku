import { cn } from "@/lib/utils";
import type { DotKind } from "@/lib/status";

const color: Record<DotKind, string> = {
  alive: "bg-primary",
  warn: "bg-warn",
  dead: "bg-destructive",
  idle: "bg-muted-foreground",
};

export function StatusDot({ kind, className }: { kind: DotKind; className?: string }) {
  return (
    <span className={cn("relative inline-block h-2.5 w-2.5", className)}>
      <span className={cn("absolute inset-0", color[kind])} />
      {kind === "alive" && (
        <span className="absolute inset-0 animate-ping bg-primary/60 motion-reduce:hidden" />
      )}
    </span>
  );
}
