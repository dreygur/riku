// @ts-check
// Markup for an app card's header: health badge, meta line and crash alert.
// Pure string builders: every server value passes through esc() or num().
import { esc, fmtAgo, fmtBytes, isOn, num } from "../format.js";
import { I } from "../icons.js";
import { workerName } from "./workers.js";

/** @param {import("../state.js").App} a */
export const domainsOf = (a) => String((a.routing && a.routing.RIKU_DOMAINS) || "").split(/[\s,]+/).filter(Boolean);

/** @param {string} kind @param {string} html trusted markup */
const badge = (kind, html) => `<span class="badge ${kind}">${html}</span>`;

/** @param {string} router @param {string} word @param {string} cls */
function routeTag(router, word, cls) {
  return `<span>${I.server}${esc(router)} <span class="${cls}">${esc(word)}</span></span>`;
}

/** @param {import("../state.js").Route} [route] */
export function routeHtml(route) {
  const r = route || {};
  if (r.router === "nginx") {
    if (r.enabled) { return routeTag("nginx", "live", "c-ok"); }
    return r.config_exists ? routeTag("nginx", "staged", "c-warn") : routeTag("nginx", "off", "c-dim");
  }
  if (r.router === "caddy") {
    return r.site ? routeTag("caddy", "configured", "c-ok") : routeTag("caddy", "off", "c-dim");
  }
  if (r.router === "plugin") { return routeTag(r.name, "plugin", "c-dim"); }
  return `<span title="${esc(r.error || "")}">${I.server}router <span class="c-bad">unknown</span></span>`;
}

/**
 * @param {import("../state.js").App} a
 * @param {import("../state.js").Release | undefined} cur current release
 * @param {boolean} busy a deploy holds the lock
 */
export function metaHtml(a, cur, busy) {
  const https = isOn((a.routing || {}).RIKU_HTTPS_ONLY);
  const doms = domainsOf(a);
  const parts = [];
  if (doms.length) {
    const d = doms[0];
    parts.push(`<a href="${https ? "https" : "http"}://${esc(d)}" target="_blank" rel="noopener noreferrer">${https ? I.https : ""}${esc(d)}${https ? "" : '<span class="tag-http">http</span>'}${I.ext}</a>`);
    if (doms.length > 1) {
      parts.push(`<span class="c-dim" title="${esc(doms.slice(1).join(", "))}">+${doms.length - 1} more</span>`);
    }
  } else {
    parts.push('<span class="c-dim">no domain</span>');
  }
  parts.push(routeHtml(a.route));
  if (busy) {
    parts.push(`<span>${I.commit}${cur ? `<b>${esc(String(cur.sha).slice(0, 8))}</b><span class="c-dim">→</span>` : ""}<span class="c-warn">deploying</span></span>`);
  } else if (cur) {
    parts.push(`<span>${I.commit}<b>${esc(String(cur.sha).slice(0, 8))}</b> deployed ${esc(fmtAgo(Date.now() / 1000 - num(cur.ts)))}</span>`);
  }
  return parts.join("");
}

/**
 * @param {boolean} busy
 * @param {import("./workers.js").WorkerState[]} st
 */
export function healthBadge(busy, st) {
  const run = st.filter((s) => s.cls === "alive").length;
  const total = st.length;
  if (busy) { return badge("warn", I.spin + "Deploying"); }
  if (!total || st.every((s) => s.cls === "off")) { return badge("", '<span class="g-ring"></span>Stopped'); }
  if (st.some((s) => s.cls === "dead")) { return badge("bad", `<span class="g-sq"></span>Degraded · ${run}/${total}`); }
  if (st.some((s) => s.cls === "warn")) { return badge("warn", `${I.spin}Starting · ${run}/${total}`); }
  return badge("ok", `<span class="g-sq"></span>Healthy · ${run}/${total}`);
}

/** @param {import("../state.js").Worker} w */
function oomText(w) {
  const limit = num(w.memory_limit_bytes);
  return limit ? `was killed for exceeding its ${fmtBytes(limit)} memory limit` : "was killed for exceeding its memory limit";
}

/**
 * The alert names the first dead worker and counts the rest.
 * @param {import("../state.js").Worker[]} ws
 * @param {import("./workers.js").WorkerState[]} st states, index-aligned with ws
 * @param {number} dead how many workers are dead
 */
export function alertHtml(ws, st, dead) {
  const i = st.findIndex((s) => s.cls === "dead");
  const w = ws[i], rc = num(w.restart_count);
  const what = st[i].oom ? oomText(w) : "crashed";
  const restarts = rc ? ` and has restarted ${rc} time${rc === 1 ? "" : "s"}` : "";
  const more = dead > 1 ? ` ${dead - 1} other worker${dead > 2 ? "s are" : " is"} also down.` : "";
  const name = esc(workerName(w));
  return `${I.alert}<p><b>${name}</b> ${what}${restarts}.${more}</p>
      <button type="button" class="link" data-do="logs" data-worker="${name}">View ${name} logs</button>`;
}
