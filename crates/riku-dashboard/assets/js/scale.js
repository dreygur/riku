// @ts-check
// The web-process stepper and the per-service steppers on container cards.
// A value differing from what runs now marks the stepper dirty and enables
// Apply.
import { MAX_SCALE } from "./render/apps.js";

/** @typedef {import("./state.js").Card} Card */

/**
 * @param {Card} c
 * @param {number} n requested web processes, clamped to 0..MAX_SCALE
 * @param {boolean} [fromInput] the value came from typing, so leave the input alone when it was already in range
 */
export function setScale(c, n, fromInput) {
  const clamped = Math.max(0, Math.min(MAX_SCALE, n | 0));
  c.scaleVal = clamped === c.webNow ? null : clamped;
  syncScale(c, fromInput && clamped === n);
}

/**
 * @param {Card} c
 * @param {boolean} [keepInput] leave the input's text as typed
 */
export function syncScale(c, keepInput) {
  const el = c.el;
  const input = /** @type {HTMLInputElement} */ (el.querySelector("[data-scale]"));
  const apply = /** @type {HTMLButtonElement} */ (el.querySelector('[data-do="scale"]'));
  const reset = /** @type {HTMLButtonElement} */ (el.querySelector('[data-do="reset"]'));
  if (!input || !apply || !reset) { return; }
  const locked = c.scaleLocked;
  if (locked) { c.scaleVal = null; }
  const val = c.scaleVal == null ? c.webNow : c.scaleVal;
  const dirty = c.scaleVal != null;
  if (!keepInput) { input.value = String(val); }
  input.disabled = locked;
  el.querySelector("[data-stepper]").classList.toggle("dirty", dirty);
  /** @type {HTMLButtonElement} */ (el.querySelector('[data-step="-1"]')).disabled = locked || val <= 0;
  /** @type {HTMLButtonElement} */ (el.querySelector('[data-step="1"]')).disabled = locked || val >= MAX_SCALE;
  reset.hidden = !dirty;
  if (apply.hasAttribute("aria-busy")) { return; }
  apply.disabled = !dirty;
  apply.classList.toggle("primary", dirty);
  apply.innerHTML = dirty ? `Apply <span class="delta">${c.webNow} → ${c.scaleVal}</span>` : "Apply";
}

/**
 * Records a pending copy count for a container service and enables its
 * Apply button when the count differs from the copies running now.
 * @param {Card} c
 * @param {string} svc
 * @param {number} n
 */
export function setServiceScale(c, svc, n) {
  if (!c.svcScale) { c.svcScale = {}; }
  c.svcScale[svc] = n;
  const apply = /** @type {HTMLButtonElement} */ (c.el.querySelector(`[data-do="svc-scale"][data-service="${svc}"]`));
  if (apply) {
    const count = (c.data && c.data.workers ? c.data.workers.filter((w) => w.kind === svc).length : 0);
    const dirty = n !== count;
    apply.disabled = !dirty;
    apply.classList.toggle("primary", dirty);
  }
}
