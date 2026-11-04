// @ts-check
// The release select on each card, fed by /api/apps/:app/releases.
import { fetchReleases } from "./api.js";
import { updateCard } from "./card.js";
import { esc, fmtWhen, num } from "./format.js";
import { session } from "./state.js";

const MAX_RELEASES = 20;

/**
 * The releases API lists oldest first; the select wants newest first.
 * @param {import("./state.js").Card} c
 */
export async function loadReleases(c) {
  try {
    const res = await fetchReleases(c.app, session.token);
    if (res.ok) {
      const rels = await res.json();
      c.releases = Array.isArray(rels) ? rels.slice().reverse().slice(0, MAX_RELEASES) : [];
    }
  } catch (_) {}
  renderReleases(c);
  if (c.data) { updateCard(c, c.data); }
}

/**
 * Fills the select with older releases, keeping the user's pick across
 * refreshes when it still exists.
 * @param {import("./state.js").Card} c
 */
export function renderReleases(c) {
  const sel = /** @type {HTMLSelectElement} */ (c.el.querySelector("[data-rel]"));
  const prev = sel.value;
  if (!c.releases.length) {
    sel.innerHTML = '<option value="">no releases yet</option>';
    sel.disabled = true;
    return;
  }
  sel.disabled = false;
  sel.innerHTML = c.releases.map((r, i) => {
    const sha = String(r.sha || "");
    return i === 0
      ? `<option value="" disabled>${esc(sha.slice(0, 8))} · current</option>`
      : `<option value="${esc(sha)}">${esc(sha.slice(0, 8))} · ${esc(fmtWhen(num(r.ts)))}</option>`;
  }).join("");
  const keep = prev && c.releases.some((r, i) => i && r.sha === prev);
  sel.value = keep ? prev : (c.releases[1] ? String(c.releases[1].sha) : "");
}
