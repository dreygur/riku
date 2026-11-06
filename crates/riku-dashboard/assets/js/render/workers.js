// @ts-check
// Markup for the worker table rows. Pure string builders: every server value
// passes through esc() or num() before it is spliced in.
import { esc, fmtBytes, fmtCpu, num } from "../format.js";
import { I } from "../icons.js";

const NO_VALUE = "-";

/**
 * @typedef {"alive" | "warn" | "dead" | "off"} StateClass
 * @typedef {{ cls: StateClass, label: string, oom?: boolean }} WorkerState
 */

/**
 * @param {unknown} s worker status from the supervisor
 * @returns {WorkerState}
 */
export const stateOf = (s) => {
  const status = String(s || "").toLowerCase();
  if (status === "running") { return { cls: "alive", label: "running" }; }
  if (status === "starting" || status === "restarting") { return { cls: "warn", label: status }; }
  if (status === "crashed") { return { cls: "dead", label: "crashed" }; }
  if (status === "oom_killed") { return { cls: "dead", label: "oom-killed", oom: true }; }
  return { cls: "off", label: status || "stopped" };
};

/** @param {StateClass} cls */
export const glyph = (cls) =>
  cls === "alive" ? '<span class="g-dot"></span>' :
  cls === "warn" ? I.spin :
  cls === "dead" ? I.cross : '<span class="g-off"></span>';

/** @param {import("../state.js").Worker} w */
export const workerName = (w) => `${w.kind}.${w.ordinal}`;

/** @param {number} used @param {number} limit */
function limitMeter(used, limit) {
  const pct = Math.min(100, (used / limit) * 100);
  const lvl = pct >= 90 ? "crit" : pct >= 75 ? "warn" : "";
  return `<span role="cell" class="mem ${lvl}">
        <span class="meter" role="img" aria-label="${Math.round(pct)}% of memory limit"><i style="width:${pct.toFixed(1)}%"></i></span>
        <span class="mem-val">${fmtBytes(used)} <span>/ ${fmtBytes(limit)}</span></span>
      </span>`;
}

/** @param {number} used @param {number} peak */
function peakMeter(used, peak) {
  const pct = peak ? Math.min(100, (used / peak) * 100) : 0;
  return `<span role="cell" class="mem">
        <span class="meter" role="img" aria-label="memory"><i style="width:${pct.toFixed(1)}%"></i></span>
        <span class="mem-val">${fmtBytes(used)}</span>
      </span>`;
}

/**
 * /api/state sends no memory limit today (RIKU_MAX_MEMORY_MB is unset by
 * default, so most workers have none). Without one the bar is relative to
 * the app's largest worker and carries no warn or crit level.
 * @param {import("../state.js").Worker} w
 * @param {number} peak
 */
function memCell(w, peak) {
  const used = num(w.memory_bytes);
  const limit = num(w.memory_limit_bytes);
  return limit ? limitMeter(used, limit) : peakMeter(used, peak);
}

/**
 * @param {import("../state.js").Worker} w
 * @param {number} peak largest memory_bytes among the app's workers
 */
export function workerRow(w, peak) {
  const isContainer = w.unit === "container";
  const st = stateOf(w.status);
  const rc = num(w.restart_count);
  const rlvl = rc >= 4 ? "crit" : "warn";
  const live = isContainer ? st.cls === "alive" || st.cls === "warn" : !!w.pid;
  const pidOrContainer = isContainer ? (w.container_id ? esc(w.container_id) : NO_VALUE) : (w.pid ? esc(w.pid) : NO_VALUE);
  const cpuDisplay = isContainer
    ? (w.cpu_percent != null ? esc(w.cpu_percent.toFixed(1)) + "%" : NO_VALUE)
    : (w.pid ? fmtCpu(w.cpu_time_ms) : NO_VALUE);
  const stateLabel = isContainer && w.state_detail ? w.state_detail : st.label;
  const nameExtra = isContainer && w.image ? `<span class="dim" title="${esc(w.image)}"> · ${esc(w.image)}</span>` : "";

  return `<div class="tr ${st.cls === "dead" ? "is-bad" : ""}" role="row">
      <span role="cell" class="w-name">${esc(workerName(w))}${nameExtra}</span>
      <span role="cell"><span class="state ${st.cls}">${glyph(st.cls)}${esc(stateLabel)}</span></span>
      ${memCell(w, peak)}
      <span role="cell" class="${live ? "muted" : "dim"}">${cpuDisplay}</span>
      <span role="cell" class="${live ? "muted" : "dim"}">${pidOrContainer}</span>
      <span role="cell" class="r">${rc ? `<span class="rc-badge ${rlvl}" title="${rc} restarts">${I.restartSm}${rc}</span>` : '<span class="dim">0</span>'}</span>
    </div>`;
}
