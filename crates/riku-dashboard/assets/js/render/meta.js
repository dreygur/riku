// @ts-check
// Markup for an app card's header: health badge, meta line and crash alert.
// Pure string builders: every server value passes through esc() or num().
import { esc, fmtAgo, fmtBytes, num } from "../format.js";
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
    if (r.config_known === false) { return routeTag("nginx", "unknown", "c-dim"); }
    return r.config_exists ? routeTag("nginx", "staged", "c-warn") : routeTag("nginx", "off", "c-dim");
  }
  if (r.router === "caddy") {
    if (r.site_known === false) { return routeTag("caddy", "unknown", "c-dim"); }
    return r.site ? routeTag("caddy", "configured", "c-ok") : routeTag("caddy", "off", "c-dim");
  }
  if (r.router === "plugin") { return routeTag(r.name, "plugin", "c-dim"); }
  return `<span title="${esc(r.error || "")}">${I.server}router <span class="c-bad">unknown</span></span>`;
}

/** Tags for the url labels the push output names; a domain needs none. */
const URL_TAGS = { lan: "LAN", public: "public" };

/**
 * A link to one live url. Only http and https urls are linked.
 * @param {import("../state.js").LiveUrl} u
 */
function urlLink(u) {
  const url = String(u.url || "");
  const parts = /^(https?):\/\/(.+)$/.exec(url);
  if (!parts) {
    return `<span>${esc(url)}</span>`;
  }
  const https = parts[1] === "https";
  const shown = parts[2];
  const tag = URL_TAGS[/** @type {"lan" | "public"} */ (u.label)];
  return `<a href="${esc(url)}" target="_blank" rel="noopener noreferrer">${https ? I.https : ""}${esc(shown)}${https ? "" : '<span class="tag-http">http</span>'}${tag ? `<span class="tag-http">${tag}</span>` : ""}${I.ext}</a>`;
}

/**
 * @param {import("../state.js").App} a
 * @param {import("../state.js").Release | undefined} cur current release
 * @param {boolean} busy a deploy holds the lock
 */
export function metaHtml(a, cur, busy) {
  const urls = a.urls || [];
  const parts = [];
  if (urls.length) {
    parts.push(urlLink(urls[0]));
    if (urls.length > 1) {
      parts.push(`<span class="c-dim" title="${esc(urls.slice(1).map((u) => u.url).join(", "))}">+${urls.length - 1} more</span>`);
    }
  } else {
    parts.push('<span class="c-dim">no domain</span>');
  }
  const pending = a.pending_urls || [];
  if (pending.length) {
    parts.push(`<span class="c-warn">settings changed, redeploy to use ${esc(pending.map((u) => String(u.url || "")).join(", "))}</span>`);
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
 * @param {boolean} neverDeployed no deploy recorded a release and no worker exists
 */
export function healthBadge(busy, st, neverDeployed) {
  const run = st.filter((s) => s.cls === "alive").length;
  const total = st.length;
  if (busy) { return badge("warn", I.spin + "Deploying"); }
  if (neverDeployed) { return badge("", '<span class="g-ring"></span>Never deployed'); }
  if (!total || st.every((s) => s.cls === "off")) { return badge("", '<span class="g-ring"></span>Stopped'); }
  if (st.some((s) => s.cls === "dead")) { return badge("bad", `<span class="g-sq"></span>Degraded · ${run}/${total}`); }
  if (st.some((s) => s.cls === "warn")) { return badge("warn", `${I.spin}Starting · ${run}/${total}`); }
  return badge("ok", `<span class="g-sq"></span>Healthy · ${run}/${total}`);
}

/**
 * Which app controls are locked. A deploy locks them all. An app never
 * deployed has nothing to restart, stop or scale, so only Logs and Redeploy
 * stay open. An app with no worker up (all stopped) has nothing to stop or
 * scale; Restart and Redeploy bring it back.
 * @param {boolean} busy
 * @param {import("./workers.js").WorkerState[]} st
 * @param {boolean} neverDeployed
 */
export function lockedControls(busy, st, neverDeployed) {
  if (busy) { return { restart: true, redeploy: true, stop: true, scale: true }; }
  const up = st.some((s) => s.cls !== "off");
  return { restart: neverDeployed, redeploy: false, stop: !up, scale: !up };
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
