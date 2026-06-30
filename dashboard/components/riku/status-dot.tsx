import { cn } from "@/lib/utils";
import type { DotKind } from "@/lib/status";

const color: Record<DotKind, string> = {
  alive: "bg-[#3fd07f]",
  warn: "bg-[#e8b54a]",
  dead: "bg-[#f25f5c]",
  idle: "bg-[#6b7480]",
};

export function StatusDot({ kind, className }: { kind: DotKind; className?: string }) {
  return (
    <span className={cn("relative inline-block h-2.5 w-2.5", className)}>
      <span className={cn("absolute inset-0", color[kind])} />
      {kind === "alive" && (
        <span className="absolute inset-0 animate-ping bg-[#3fd07f]/60 motion-reduce:hidden" />
      )}
    </span>
  );
}
