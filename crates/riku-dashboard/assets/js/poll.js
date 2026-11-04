// @ts-check
// The refresh loop: polls /api/state, schedules the next poll, and keeps the
// unreachable banner and "synced" stamp current.
import { fetchState } from "./api.js";
import { $ } from "./dom.js";
import { fmtAgo } from "./format.js";
import { render, setSup, showLocked } from "./page.js";
import { session } from "./state.js";

const POLL_MS = 4000, RETRY_MS = 4000;

/** @type {import("./api.js").Failure | null} null while polls succeed */
let failure = null;
let lastOk = 0, nextAt = 0;
/** @type {ReturnType<typeof setTimeout> | undefined} */
let timer;

/** @param {number} ms */
function schedule(ms) { clearTimeout(timer); nextAt = Date.now() + ms; timer = setTimeout(load, ms); }

/** Polls again shortly, so an action's effect shows without waiting a full cycle. */
export const soon = () => schedule(600);

/** @param {import("./api.js").Failure | null} f */
function setFailure(f) {
  failure = f;
  $("#banner").hidden = !f;
  $("#apps").classList.toggle("stale", !!f && session.mode === "apps");
  if (f) { setSup(f.status ? "error" : "down", f); }
}

/**
 * Re-render the locked panel only on the way in, or when the token flips
 * between missing and rejected, so a poll never steals focus from the
 * token field.
 */
function onLocked() {
  const rejected = !!session.token;
  setFailure(null);
  if (session.mode !== "locked" || rejected !== session.tokenRejected) {
    session.tokenRejected = rejected;
    showLocked();
  }
}

export async function load() {
  clearTimeout(timer);
  const got = await fetchState(session.token);
  if (got.locked) {
    onLocked();
    schedule(POLL_MS);
  } else if (got.failure) {
    setFailure(got.failure);
    schedule(RETRY_MS);
  } else {
    lastOk = Date.now();
    setFailure(null);
    schedule(POLL_MS);
    render(got.data);
  }
  tick();
}

function staleNote() {
  if (!lastOk || session.mode !== "apps") { return ""; }
  const ago = fmtAgo((Date.now() - lastOk) / 1000);
  return ago === "just now" ? " Showing the last data received." : " Showing data from " + ago + ".";
}

/** @param {import("./api.js").Failure} f */
function failureText(f) {
  if (!f.status) { return "Can't reach the supervisor."; }
  return `The supervisor answered HTTP ${f.status}${f.body ? ": " + f.body : "."}`;
}

function tick() {
  $("#synced").textContent = lastOk && !failure ? "synced " + fmtAgo((Date.now() - lastOk) / 1000) : "";
  if (!failure) { return; }
  $("#bannerText").textContent = failureText(failure) + staleNote();
  const s = Math.max(0, Math.ceil((nextAt - Date.now()) / 1000));
  $("#bannerRetry").textContent = s ? `retry in ${s}s` : "retrying...";
}

/** Wires the banner's retry button and starts the once-a-second tick. */
export function initPoll() {
  $("#retryBtn").addEventListener("click", () => { $("#bannerRetry").textContent = "retrying..."; load(); });
  setInterval(tick, 1000);
}
