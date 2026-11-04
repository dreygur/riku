// @ts-check
// Renders a /api/state answer into the page: the supervisor pill, the
// summary chips, and one card per app, or the empty and locked panels.
import { renderAuth, wireTokenForms } from "./auth.js";
import { createCard, updateCard } from "./card.js";
import { $ } from "./dom.js";
import { esc, fmtDur, num } from "./format.js";
import { closeLogs, renderSeg } from "./logs/drawer.js";
import { L } from "./logs/stream.js";
import { emptyHtml, lockedHtml } from "./render/panels.js";
import { stateOf } from "./render/workers.js";
import { cards, session } from "./state.js";

/**
 * @param {"live" | "down" | "error" | "locked"} state
 * @param {import("./state.js").StateData | import("./api.js").Failure} [data]
 *   the state for "live", the failure for "error"
 */
export function setSup(state, data) {
  const el = $("#sup");
  el.className = "sup " + state;
  const t = $("#supText");
  if (state === "live" && data) {
    const d = /** @type {import("./state.js").StateData} */ (data);
    const up = d.supervisor_uptime_seconds == null ? "uptime unknown" : "up " + fmtDur(d.supervisor_uptime_seconds);
    const eng = d.engine ? `<span class="sep">/</span>${esc(d.engine.name)} ${esc(d.engine.version)}` : "";
    t.innerHTML = `<b>Live</b><span class="sep">/</span>v${esc(d.riku_version)}<span class="sep">/</span>${esc(up)}${eng}`;
  } else if (state === "down") {
    t.innerHTML = "<b>Unreachable</b>";
  } else if (state === "error" && data) {
    t.innerHTML = `<b>HTTP ${num(/** @type {import("./api.js").Failure} */ (data).status)}</b>`;
  } else if (state === "locked") {
    t.innerHTML = "<b>Locked</b>";
  } else {
    t.textContent = "connecting...";
  }
}

/** @param {import("./state.js").App[]} apps */
function renderSummary(apps) {
  let total = 0, run = 0, warn = 0, dead = 0, off = 0;
  apps.forEach((a) => (a.workers || []).forEach((w) => {
    total++;
    const s = stateOf(w.status).cls;
    if (s === "alive") { run++; } else if (s === "warn") { warn++; } else if (s === "dead") { dead++; } else { off++; }
  }));
  $("#pageSub").textContent = `${apps.length} app${apps.length === 1 ? "" : "s"} on this host · ${total} worker${total === 1 ? "" : "s"}`;
  const chips = [`<span class="sum ok"><i></i><b>${run}</b> running</span>`];
  if (warn) { chips.push(`<span class="sum warn"><i></i><b>${warn}</b> starting</span>`); }
  if (dead) { chips.push(`<span class="sum bad"><i></i><b>${dead}</b> failed</span>`); }
  if (off) { chips.push(`<span class="sum"><i></i><b>${off}</b> stopped</span>`); }
  $("#summary").innerHTML = chips.join("");
  $("#pageHead").hidden = false;
}

/** @param {import("./state.js").StateData} data */
export function render(data) {
  setSup("live", data);
  const apps = data.apps || [];
  const wrap = $("#apps");
  if (!apps.length) {
    if (session.mode !== "empty") { cards.clear(); wrap.innerHTML = emptyHtml(location.hostname); session.mode = "empty"; renderAuth(); }
    $("#pageHead").hidden = true;
    if (L.app) { closeLogs(); }
    return;
  }
  if (session.mode !== "apps") { wrap.innerHTML = ""; cards.clear(); session.mode = "apps"; renderAuth(); }

  const seen = new Set();
  apps.forEach((a, i) => {
    seen.add(a.app);
    let c = cards.get(a.app);
    if (!c) { c = createCard(a.app); cards.set(a.app, c); }
    updateCard(c, a);
    if (wrap.children[i] !== c.el) { wrap.insertBefore(c.el, wrap.children[i] || null); }
  });
  for (const [k, c] of cards) {
    if (!seen.has(k)) { c.el.remove(); cards.delete(k); }
  }

  renderSummary(apps);
  if (L.app) {
    if (cards.has(L.app)) { renderSeg(); } else { closeLogs(); }
  }
}

export function showLocked() {
  setSup("locked");
  $("#pageHead").hidden = true;
  if (L.app) { closeLogs(); }
  cards.clear();
  const wasLocked = session.mode === "locked";
  session.mode = "locked";
  renderAuth();
  const wrap = $("#apps");
  const panelInput = () => /** @type {HTMLInputElement} */ ($("#tokInputPanel"));
  const typed = wasLocked && panelInput() ? panelInput().value : "";
  wrap.innerHTML = lockedHtml(session.tokenRejected);
  wireTokenForms(wrap);
  if (typed) { panelInput().value = typed; }
}

/** Wires the Copy buttons on the empty-state panel. */
export function initCopyButtons() {
  $("#apps").addEventListener("click", async (e) => {
    const b = /** @type {HTMLElement} */ (/** @type {HTMLElement} */ (e.target).closest("[data-copy]"));
    if (!b) { return; }
    try { await navigator.clipboard.writeText(b.dataset.copy); b.textContent = "Copied"; }
    catch (_) { b.textContent = "Select & copy"; }
    setTimeout(() => { b.textContent = "Copy"; }, 1600);
  });
}
