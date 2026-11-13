// @ts-check
// The per kind process steppers and the per-service steppers on container
// cards. A value differing from what runs now marks the stepper dirty and
// enables Apply.
import { esc } from "./format.js";
import { MAX_SCALE } from "./render/apps.js";

/** @typedef {import("./state.js").Card} Card */

/** @param {Card} c @param {string} kind */
const nowOf = (c, kind) => c.kindNow.get(kind) || 0;

/**
 * The pending count for `kind`, or what runs now when nothing is pending.
 * @param {Card} c
 * @param {string} kind
 */
export const valueOf = (c, kind) => (c.kindScale.has(kind) ? c.kindScale.get(kind) : nowOf(c, kind));

/**
 * @param {Card} c
 * @param {string} kind
 * @param {number} n requested processes of `kind`, clamped to 0..MAX_SCALE
 * @param {boolean} [fromInput] the value came from typing, so leave the input alone when it was already in range
 */
export function setScale(c, kind, n, fromInput) {
  const clamped = Math.max(0, Math.min(MAX_SCALE, n | 0));
  if (clamped === nowOf(c, kind)) { c.kindScale.delete(kind); } else { c.kindScale.set(kind, clamped); }
  syncScale(c, fromInput && clamped === n ? kind : undefined);
}

/**
 * @param {Card} c
 * @param {string} [typedKind] leave this kind's input text as typed
 */
export function syncScale(c, typedKind) {
  const el = c.el;
  const apply = /** @type {HTMLButtonElement} */ (el.querySelector('[data-do="scale"]'));
  const reset = /** @type {HTMLButtonElement} */ (el.querySelector('[data-do="reset"]'));
  if (!apply || !reset) { return; }
  const locked = c.scaleLocked;
  if (locked) { c.kindScale.clear(); }
  el.querySelectorAll("[data-stepper]").forEach((node) => {
    const stepper = /** @type {HTMLElement} */ (node);
    const kind = stepper.dataset.stepper;
    const val = valueOf(c, kind);
    const input = /** @type {HTMLInputElement} */ (stepper.querySelector("[data-scale]"));
    if (kind !== typedKind) { input.value = String(val); }
    input.disabled = locked;
    stepper.classList.toggle("dirty", c.kindScale.has(kind));
    /** @type {HTMLButtonElement} */ (stepper.querySelector('[data-step="-1"]')).disabled = locked || val <= 0;
    /** @type {HTMLButtonElement} */ (stepper.querySelector('[data-step="1"]')).disabled = locked || val >= MAX_SCALE;
  });
  const dirty = c.kindScale.size > 0;
  reset.hidden = !dirty;
  if (apply.hasAttribute("aria-busy")) { return; }
  apply.disabled = !dirty;
  apply.classList.toggle("primary", dirty);
  const changes = [...c.kindScale].map(([kind, n]) => `${esc(kind)} ${nowOf(c, kind)} → ${n}`).join(", ");
  apply.innerHTML = dirty ? `Apply <span class="delta">${changes}</span>` : "Apply";
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
