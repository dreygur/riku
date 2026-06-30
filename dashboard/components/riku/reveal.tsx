/**
 * Subtle staggered fade-up on mount. Pure CSS (tw-animate-css) — no JS
 * animation runtime, so it works under strict CSP (no unsafe-eval).
 */
export function Reveal({ children, i = 0 }: { children: React.ReactNode; i?: number }) {
  return (
    <div
      className="fade-in slide-in-from-bottom-2 animate-in fill-mode-backwards duration-300"
      style={{ animationDelay: `${Math.min(i, 8) * 40}ms` }}
    >
      {children}
    </div>
  );
}
