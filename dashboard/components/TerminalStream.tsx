"use client";

import * as React from "react";

const MAX_HISTORY = 2_000;

export interface TerminalStreamProps {
  app: string;
  filter?: string;
  className?: string;
}

export function TerminalStream({ app, filter, className }: TerminalStreamProps) {
  const [lines, setLines] = React.useState<string[]>([]);
  const [connected, setConnected] = React.useState(false);
  const [autoScroll, setAutoScroll] = React.useState(true);
  const endRef = React.useRef<HTMLDivElement>(null);
  const containerRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    let es: EventSource | null = null;
    try {
      es = new EventSource(`/api/logs/stream?app=${encodeURIComponent(app)}`);
      es.onopen = () => setConnected(true);
      es.onerror = () => setConnected(false);
      es.onmessage = (e: MessageEvent<string>) => {
        const { line } = JSON.parse(e.data) as { line: string };
        setLines((prev) => {
          const next = [...prev, line];
          return next.length > MAX_HISTORY ? next.slice(-MAX_HISTORY) : next;
        });
      };
    } catch {
      setConnected(false);
    }
    return () => {
      es?.close();
      setConnected(false);
    };
  }, [app]);

  React.useEffect(() => {
    if (autoScroll && endRef.current) {
      endRef.current.scrollIntoView({ behavior: "instant" });
    }
  }, [lines, autoScroll]);

  const handleScroll = React.useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 32;
    if (!atBottom) setAutoScroll(false);
  }, []);

  const filtered = React.useMemo(() => {
    if (!filter) return lines;
    const lower = filter.toLowerCase();
    return lines.filter((l) => l.toLowerCase().includes(lower));
  }, [lines, filter]);

  return (
    <div
      className={`flex flex-col h-full border border-primary-burgundy ${className ?? ""}`}
    >
      {/* Header bar */}
      <div className="flex items-center justify-between border-b border-primary-burgundy px-3 py-2 shrink-0">
        <div className="flex items-center gap-2">
          <span className="text-xs font-bold tracking-wide uppercase text-foreground-muted">
            LOG_STREAM_BUFFER
          </span>
          <span
            className={`text-xs tabular ${
              connected ? "text-accent-orange" : "text-foreground-dim"
            }`}
          >
            [{connected ? "LIVE" : "OFFLINE"}]
          </span>
        </div>
        <div className="flex items-center gap-3">
          <span className="text-xs text-foreground-dim tabular">
            {filtered.length} lines
          </span>
          <button
            onClick={() => {
              setAutoScroll((prev) => !prev);
              if (!autoScroll && endRef.current) {
                endRef.current.scrollIntoView({ behavior: "instant" });
              }
            }}
            className={`rounded-none text-xs font-bold border px-2 py-0.5 transition-colors ${
              autoScroll
                ? "border-accent-orange text-accent-orange bg-status-running"
                : "border-primary-burgundy text-foreground-muted hover:text-foreground-dark hover:bg-primary-burgundy/10"
            }`}
          >
            [{autoScroll ? "SCROLL:ON" : "SCROLL:OFF"}]
          </button>
        </div>
      </div>

      {/* Terminal body */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto bg-background-dark p-3 min-h-0"
      >
        <pre className="text-xs leading-relaxed text-foreground-dark/80 whitespace-pre-wrap break-all">
          {filtered.length === 0 ? (
            <span className="text-foreground-dim">
              {connected
                ? "waiting for log output..."
                : "connecting to log stream..."}
            </span>
          ) : (
            filtered.map((line, i) => (
              <div key={i} className="hover:bg-white/5">
                <span className="text-accent-orange/40 mr-2">❯</span>
                {line}
              </div>
            ))
          )}
          <div ref={endRef} />
        </pre>
      </div>
    </div>
  );
}
