// @ts-check
// One app card: built once per app, then updated in place on every poll so
// focus, typed scale values and open selects survive a refresh.
import { wire } from "./card-events.js";
import { num } from "./format.js";
import { loadReleases, renderReleases } from "./releases.js";
import { cardHtml } from "./render/apps.js";
import { serviceControlsHtml } from "./render/containers.js";
import { alertHtml, healthBadge, lockedControls, metaHtml } from "./render/meta.js";
import { stateOf, workerRow } from "./render/workers.js";
import { syncScale } from "./scale.js";
import { nextCardId } from "./state.js";

/** @typedef {import("./state.js").Card} Card */

/** @param {string} app */
export function createCard(app) {
  const id = nextCardId();
  const el = document.createElement("section");
  el.className = "app";
  el.setAttribute("aria-labelledby", id);
  el.innerHTML = cardHtml(id, app);
  /** @type {Card} */
  const c = { app, id, el, data: null, releases: [], webNow: 0, scaleVal: null, scaleLocked: false, wasBusy: false, svcScale: {} };
  wire(c);
  loadReleases(c);
  return c;
}

/**
 * @param {Card} c
 * @param {import("./state.js").Worker[]} ws
 */
function renderControls(c, ws) {
  const controls = c.el.querySelector("[data-controls]");
  if (ws.some((w) => w.unit === "container")) {
    controls.innerHTML = serviceControlsHtml(c.id, ws, c.svcScale);
    renderReleases(c);
    return;
  }
  c.webNow = ws.filter((w) => w.kind === "web").length;
  if (c.scaleVal === c.webNow) { c.scaleVal = null; }
  syncScale(c);
}

/**
 * Locks the controls that must not run now (see lockedControls), leaving
 * any button mid-action alone.
 * @param {Card} c
 * @param {boolean} busy a deploy holds the lock
 * @param {ReturnType<typeof lockedControls>} locks
 */
function lockControls(c, busy, locks) {
  const el = c.el;
  /** @type {HTMLElement} */ (el.querySelector("[data-controls]")).hidden = busy;
  /** @type {HTMLElement} */ (el.querySelector("[data-busynote]")).hidden = !busy;
  /** @param {string} sel @param {boolean} locked */
  const lock = (sel, locked) => el.querySelectorAll(sel).forEach((b) => {
    if (!b.hasAttribute("aria-busy")) { /** @type {HTMLButtonElement} */ (b).disabled = locked; }
  });
  lock('[data-do="restart"]', locks.restart);
  lock('[data-do="redeploy"]', locks.redeploy);
  lock('[data-do="stop"]', locks.stop);
  lock('[data-do="svc-restart"],[data-do="svc-stop"]', busy);
  const rb = /** @type {HTMLButtonElement} */ (el.querySelector('[data-do="rollback"]'));
  if (rb && !rb.hasAttribute("aria-busy")) { rb.disabled = busy || c.releases.length < 2; }
  c.scaleLocked = locks.scale;
  syncScale(c);
}

/**
 * @param {Card} c
 * @param {import("./state.js").App} a the app's /api/state entry
 */
export function updateCard(c, a) {
  c.data = a;
  const el = c.el;
  const busy = String(a.deploy_lock || "").toLowerCase() === "held";
  if (c.wasBusy && !busy) { loadReleases(c); }
  c.wasBusy = busy;

  const ws = (a.workers || []).slice().sort((x, y) =>
    String(x.kind).localeCompare(String(y.kind)) || (num(x.ordinal) - num(y.ordinal)));
  const st = ws.map((w) => stateOf(w.status));
  const dead = st.filter((s) => s.cls === "dead").length;

  const neverDeployed = a.released === false && !ws.length;
  el.querySelector("[data-health]").innerHTML = healthBadge(busy, st, neverDeployed);
  el.classList.toggle("is-busy", busy);
  el.classList.toggle("is-bad", !busy && dead > 0);
  el.setAttribute("aria-busy", busy ? "true" : "false");

  el.querySelector("[data-meta]").innerHTML = metaHtml(a, c.releases[0], busy);

  const al = /** @type {HTMLElement} */ (el.querySelector("[data-alert]"));
  if (dead && !busy) {
    al.innerHTML = alertHtml(ws, st, dead);
    al.hidden = false;
  } else {
    al.hidden = true; al.innerHTML = "";
  }

  const peak = ws.reduce((max, w) => Math.max(max, num(w.memory_bytes)), 0);
  el.querySelector("[data-rows]").innerHTML = ws.length
    ? ws.map((w) => workerRow(w, peak)).join("")
    : '<div class="tr empty-row" role="row"><span role="cell">No workers running.</span></div>';

  renderControls(c, ws);
  lockControls(c, busy, lockedControls(busy, st, neverDeployed));
}
