export function Sparkline({
  data,
  max,
  stroke = "var(--color-info)",
  width = 140,
  height = 30,
}: {
  data: number[];
  max?: number;
  stroke?: string;
  width?: number;
  height?: number;
}) {
  if (data.length < 2) {
    return <span className="font-mono text-[11px] text-muted-foreground">collecting…</span>;
  }
  const m = max ?? Math.max(...data, 1);
  const step = width / (data.length - 1);
  const pts = data
    .map((v, i) => `${(i * step).toFixed(1)},${(height - (v / m) * height).toFixed(1)}`)
    .join(" ");
  const areaPts = `0,${height} ${pts} ${width},${height}`;
  return (
    // `width`/`height` size the internal point coordinates only; the element
    // itself scales to its container via viewBox + w-full, so this doesn't
    // hard-overflow a narrow row the way a fixed pixel `width` attribute did.
    <svg
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
      className="block w-full min-w-0"
      style={{ height }}
      aria-hidden
    >
      <polygon points={areaPts} fill={stroke} fillOpacity={0.12} />
      <polyline points={pts} fill="none" stroke={stroke} strokeWidth={1.5} vectorEffect="non-scaling-stroke" />
    </svg>
  );
}
