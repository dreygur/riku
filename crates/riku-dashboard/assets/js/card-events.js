// @ts-check
// Event wiring for one app card: steppers, scale input, and the [data-do]
// buttons that run actions.
import { act, confirmStop, withBusy } from "./actions.js";
import { openLogs } from "./logs/drawer.js";
import { soon } from "./poll.js";
import { loadReleases } from "./releases.js";
import { MAX_SCALE } from "./render/apps.js";
import { setScale, setServiceScale, syncScale } from "./scale.js";
import { toast } from "./toast.js";

/** @typedef {import("./state.js").Card} Card */

/**
 * @param {Card} c
 * @param {string} svc
 */
const serviceInput = (c, svc) => /** @type {HTMLInputElement} */ (c.el.querySelector(`[data-svc-scale="${svc}"]`));

/**
 * @param {Card} c
 * @param {HTMLButtonElement} step a [data-svc-step] button
 */
function stepService(c, step) {
  const svc = step.dataset.service;
  const sInput = serviceInput(c, svc);
  if (!sInput) { return; }
  const cur = parseInt(sInput.value, 10) || 0;
  const next = Math.max(0, Math.min(MAX_SCALE, cur + Number(step.dataset.svcStep)));
  setServiceScale(c, svc, next);
  sInput.value = String(next);
}

/**
 * Service actions: scale, restart and stop for one container service.
 * @param {Card} c
 * @param {HTMLButtonElement} btn
 * @param {string} what
 */
async function serviceAction(c, btn, what) {
  const app = c.app, svc = btn.dataset.service;
  const path = (/** @type {string} */ verb) => `services/${encodeURIComponent(svc)}/${verb}`;
  const viewLogs = { label: "View logs", run: () => openLogs(app) };
  if (what === "svc-scale") {
    const sInput = serviceInput(c, svc);
    const n = sInput ? parseInt(sInput.value, 10) : null;
    if (n == null || isNaN(n)) { return; }
    await withBusy(c, btn, "Applying...", async () => {
      if (await act(app, path("scale"), { copies: n })) {
        toast("ok", `Scaling ${app} ${svc} to ${n}`, "Service is adjusting copies.", viewLogs);
        if (c.svcScale) { delete c.svcScale[svc]; }
        soon();
      }
    });
  } else if (what === "svc-restart") {
    await withBusy(c, btn, "Restarting...", async () => {
      if (await act(app, path("restart"))) {
        toast("ok", `Restarting ${app} ${svc}`, "Service copies restarting.", viewLogs);
        soon();
      }
    });
  } else if (what === "svc-stop") {
    await withBusy(c, btn, "Stopping...", async () => {
      if (await act(app, path("stop"))) {
        toast("ok", `Stopping ${app} ${svc}`, "Service copies stopped.");
        soon();
      }
    });
  }
}

/**
 * App-wide actions from the card header and footer.
 * @param {Card} c
 * @param {HTMLButtonElement} btn
 * @param {string} what the button's data-do
 */
async function appAction(c, btn, what) {
  const app = c.app;
  const viewLogs = { label: "View logs", run: () => openLogs(app) };
  if (what === "scale") {
    const n = c.scaleVal;
    if (n == null) { return; }
    await withBusy(c, btn, "Applying...", async () => {
      if (await act(app, "scale", { web: n })) {
        toast("ok", `Scaling ${app} web to ${n}`, "Workers appear as they start.", viewLogs);
        c.scaleVal = null; soon();
      }
    });
  } else if (what === "rollback") {
    const to = /** @type {HTMLSelectElement} */ (c.el.querySelector("[data-rel]")).value;
    await withBusy(c, btn, "Rolling back...", async () => {
      if (await act(app, "rollback", to ? { to } : {})) {
        toast("ok", `Rolling back ${app}`, to ? `To ${to.slice(0, 8)}.` : "To the previous release.", viewLogs);
        soon(); setTimeout(() => loadReleases(c), 3000);
      }
    });
  } else if (what === "stop") {
    if (!(await confirmStop(c))) { return; }
    await withBusy(c, btn, "Stopping...", async () => {
      if (await act(app, "stop")) { toast("ok", `Stopping ${app}`, "Its workers are shutting down."); soon(); }
    });
  } else if (what === "restart") {
    await withBusy(c, btn, "Restarting...", async () => {
      if (await act(app, "restart")) { toast("ok", `Restarting ${app}`, "Workers will be back in a moment.", viewLogs); soon(); }
    });
  } else if (what === "redeploy") {
    await withBusy(c, btn, "Redeploying...", async () => {
      if (await act(app, "redeploy")) {
        toast("ok", `Redeploying ${app}`, "Controls lock until the deploy finishes.", { label: "Follow output", run: () => openLogs(app) });
        soon();
      }
    });
  } else {
    await serviceAction(c, btn, what);
  }
}

/**
 * @param {Card} c
 * @param {MouseEvent} e
 */
async function onClick(c, e) {
  const target = /** @type {HTMLElement} */ (e.target);
  const step = /** @type {HTMLButtonElement} */ (target.closest("[data-step]"));
  if (step) {
    if (!step.disabled) { setScale(c, (c.scaleVal == null ? c.webNow : c.scaleVal) + Number(step.dataset.step)); }
    return;
  }
  const svcStep = /** @type {HTMLButtonElement} */ (target.closest("[data-svc-step]"));
  if (svcStep && !svcStep.disabled) {
    stepService(c, svcStep);
    return;
  }
  const btn = /** @type {HTMLButtonElement} */ (target.closest("[data-do]"));
  if (!btn || btn.disabled) { return; }
  const what = btn.dataset.do;
  if (what === "logs") { openLogs(c.app, btn.dataset.worker || ""); return; }
  if (what === "reset") { c.scaleVal = null; syncScale(c); return; }
  await appAction(c, btn, what);
}

/**
 * @param {Card} c
 * @param {HTMLInputElement} input the card's web scale input
 */
function wireScaleInput(c, input) {
  input.addEventListener("input", () => {
    const n = parseInt(input.value, 10);
    if (!isNaN(n)) { setScale(c, n, true); }
  });
  input.addEventListener("blur", () => syncScale(c));
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") { e.preventDefault(); const b = /** @type {HTMLButtonElement} */ (c.el.querySelector('[data-do="scale"]')); if (b) b.click(); }
    if (e.key === "Escape") { c.scaleVal = null; syncScale(c); }
  });
}

/** @param {Card} c */
export function wire(c) {
  const el = c.el;
  const input = /** @type {HTMLInputElement} */ (el.querySelector("[data-scale]"));
  el.addEventListener("click", (e) => onClick(c, e));
  if (input) { wireScaleInput(c, input); }
  el.addEventListener("input", (e) => {
    const sInput = /** @type {HTMLInputElement} */ (/** @type {HTMLElement} */ (e.target).closest("[data-svc-scale]"));
    if (!sInput) { return; }
    const n = parseInt(sInput.value, 10);
    if (!isNaN(n)) { setServiceScale(c, sInput.dataset.svcScale, n); }
  });
}
