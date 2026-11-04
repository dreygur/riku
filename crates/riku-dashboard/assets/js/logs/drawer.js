// @ts-check
// The log drawer: opening and closing it, the worker filter, follow, pause,
// and the footer that reports stream state.
import { $ } from "../dom.js";
import { esc } from "../format.js";
import { workerName } from "../render/workers.js";
import { cards } from "../state.js";
import { L, addLine, clearLines, con, startStream, stopStream, visible } from "./stream.js";

const drawer = $("#drawer");

/** @type {Record<import("./stream.js").StreamState, [string, string]>} */
const STREAM_LABELS = {
  live: ["live", "streaming"], connecting: ["", "connecting"], reconnecting: ["warn", "reconnecting"],
  closed: ["bad", "disconnected"], idle: ["", "idle"],
};

const atBottom = () => con.scrollHeight - con.scrollTop - con.clientHeight < 40;

let syncQueued = false;

/** Repaints the drawer chrome once per animation frame, however often it is asked. */
export function syncLogUi() {
  if (syncQueued) { return; }
  syncQueued = true;
  requestAnimationFrame(() => {
    syncQueued = false;
    $("#followBtn").setAttribute("aria-pressed", String(L.follow));
    $("#pauseBtn").textContent = L.paused ? (L.pending.length ? `Resume (${L.pending.length})` : "Resume") : "Pause";
    let [cls, txt] = STREAM_LABELS[L.stream] || STREAM_LABELS.idle;
    if (L.paused) { cls = "warn"; txt = `paused · ${L.pending.length} buffered`; }
    const ss = $("#streamState");
    ss.className = "stream " + cls;
    ss.lastElementChild.textContent = txt;
    $("#lineCount").textContent = `${L.lines.length.toLocaleString()} line${L.lines.length === 1 ? "" : "s"}`;
    const jb = $("#jumpBtn");
    jb.hidden = L.follow;
    jb.textContent = L.unseen ? `${L.unseen} new · jump to latest` : "Jump to latest";
    const empty = con.querySelector(".console-empty");
    if (!L.lines.length && !empty) {
      con.insertAdjacentHTML("afterbegin", '<div class="console-empty">Waiting for output...</div>');
    } else if (L.lines.length && empty) {
      empty.remove();
    }
  });
}

/** Rebuilds the worker filter from the card's workers and the tags seen so far. */
export function renderSeg() {
  const c = cards.get(L.app);
  const names = new Set(((c && c.data && c.data.workers) || []).map(workerName));
  L.tags.forEach((t) => names.add(t));
  const list = [...names].sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
  if (L.worker && !names.has(L.worker)) { list.push(L.worker); }
  $("#logFilter").innerHTML = [["", "all"], ...list.map((n) => [n, n])]
    .map(([v, l]) => `<button type="button" data-w="${esc(v)}" aria-pressed="${L.worker === v}">${esc(l)}</button>`).join("");
}

function applyFilter() {
  L.lines.forEach((ln) => { ln.el.hidden = !visible(ln); });
  if (L.follow) { con.scrollTop = con.scrollHeight; }
}

/**
 * @param {string} app
 * @param {string} [worker] preselects the filter, all workers when empty
 */
export function openLogs(app, worker) {
  if (L.app !== app) {
    stopStream();
    L.app = app; L.lines = []; L.tags = new Set(); L.pending = []; L.paused = false; L.follow = true; L.unseen = 0;
    con.replaceChildren();
    startStream();
  }
  L.worker = worker || "";
  $("#drawerApp").textContent = app;
  renderSeg(); applyFilter(); syncLogUi();
  drawer.removeAttribute("inert");
  drawer.classList.add("open");
  document.body.classList.add("drawer-open");
  con.focus({ preventScroll: true });
}

export function closeLogs() {
  stopStream();
  L.app = null; L.stream = "idle";
  drawer.classList.remove("open");
  drawer.setAttribute("inert", "");
  document.body.classList.remove("drawer-open");
}

function jumpToLatest() {
  L.follow = true; L.unseen = 0; con.scrollTop = con.scrollHeight; syncLogUi();
}

/** @param {KeyboardEvent} e */
function onEscape(e) {
  if (e.key !== "Escape" || !drawer.classList.contains("open")) { return; }
  if (/** @type {HTMLDialogElement} */ ($("#confirm")).open) { return; }
  const target = /** @type {HTMLElement} */ (e.target);
  if (target && target.matches && target.matches("[data-scale]")) { return; }
  const search = /** @type {HTMLInputElement} */ ($("#logQ"));
  if (target === search && search.value) { search.value = ""; L.q = ""; applyFilter(); return; }
  closeLogs();
}

/** Wires the drawer's buttons, filter, scroll tracking and Escape key. */
export function initDrawer() {
  $("#drawerClose").addEventListener("click", closeLogs);
  $("#logFilter").addEventListener("click", (e) => {
    const b = /** @type {HTMLElement} */ (/** @type {HTMLElement} */ (e.target).closest("[data-w]"));
    if (!b) { return; }
    L.worker = b.dataset.w; renderSeg(); applyFilter();
  });
  $("#logQ").addEventListener("input", (e) => { L.q = /** @type {HTMLInputElement} */ (e.target).value.trim().toLowerCase(); applyFilter(); });
  $("#followBtn").addEventListener("click", () => {
    L.follow = !L.follow;
    if (L.follow) { L.unseen = 0; con.scrollTop = con.scrollHeight; }
    syncLogUi();
  });
  $("#jumpBtn").addEventListener("click", jumpToLatest);
  $("#pauseBtn").addEventListener("click", () => {
    L.paused = !L.paused;
    if (!L.paused) { const p = L.pending; L.pending = []; p.forEach(addLine); }
    syncLogUi();
  });
  $("#clearBtn").addEventListener("click", clearLines);
  con.addEventListener("scroll", () => {
    const bottom = atBottom();
    if (bottom && !L.follow) { L.follow = true; L.unseen = 0; syncLogUi(); }
    else if (!bottom && L.follow) { L.follow = false; syncLogUi(); }
  }, { passive: true });
  document.addEventListener("keydown", onEscape);
}
