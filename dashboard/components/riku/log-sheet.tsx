"use client";

import { useEffect, useRef, useState } from "react";
import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { ScrollArea } from "@/components/ui/scroll-area";
import { api } from "@/lib/api";

type Line = { tag: string; text: string };

export function LogSheet({
  app,
  open,
  onOpenChange,
}: {
  app: string;
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const [lines, setLines] = useState<Line[]>([]);
  const boxRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    setLines([]);
    const es = new EventSource(api.logsUrl(app));
    es.addEventListener("log", (ev) => {
      const data = (ev as MessageEvent).data as string;
      const tab = data.indexOf("\t");
      const tag = tab > 0 ? data.slice(0, tab) : "";
      const text = tab > 0 ? data.slice(tab + 1) : data;
      setLines((prev) => {
        const next = prev.length > 2000 ? prev.slice(-2000) : prev.slice();
        next.push({ tag, text });
        return next;
      });
    });
    return () => es.close();
  }, [app, open]);

  useEffect(() => {
    const el = boxRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines]);

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="bottom"
        className="h-[52vh] border-t border-border bg-[#0a0c0e] p-0"
      >
        <SheetHeader className="border-b border-border px-4 py-2.5">
          <SheetTitle className="font-mono text-xs tracking-widest text-muted-foreground uppercase">
            logs · {app}
          </SheetTitle>
        </SheetHeader>
        <ScrollArea className="h-[calc(52vh-44px)]">
          <div ref={boxRef} className="px-4 py-3 font-mono text-xs leading-relaxed">
            {lines.length === 0 ? (
              <p className="text-muted-foreground">waiting for output…</p>
            ) : (
              lines.map((l, i) => (
                <div key={i} className="whitespace-pre-wrap break-words">
                  <span className="mr-3 select-none text-border">{l.tag}</span>
                  {l.text}
                </div>
              ))
            )}
          </div>
        </ScrollArea>
      </SheetContent>
    </Sheet>
  );
}
