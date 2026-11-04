// @ts-check
// The header's auth area: token form, "Controls unlocked" with Lock, or the
// note that actions are disabled on this server.
import { $ } from "./dom.js";
import { I } from "./icons.js";
import { load } from "./poll.js";
import { skeletonHtml, tokenFormHtml } from "./render/panels.js";
import { TOKEN_KEY, session, store } from "./state.js";

/**
 * Makes every [data-tok] form under `root` save its token and reload.
 * @param {ParentNode} root
 */
export function wireTokenForms(root) {
  root.querySelectorAll("[data-tok]").forEach((f) => f.addEventListener("submit", (e) => {
    e.preventDefault();
    const input = /** @type {HTMLInputElement} */ (f.querySelector("input"));
    const v = input.value.trim();
    if (!v) { input.focus(); return; }
    session.token = v; session.tokenRejected = false; store.set(TOKEN_KEY, v);
    if (session.mode === "locked") { session.mode = "loading"; $("#apps").innerHTML = skeletonHtml(); }
    renderAuth(); load();
  }));
}

export function renderAuth() {
  const el = $("#auth");
  if (session.mode === "locked") { el.innerHTML = ""; return; }
  if (session.actionsDisabled) {
    el.innerHTML = `<div class="auth-note" role="status">${I.lock}<span>Actions are disabled on this server (started without --token)</span></div>`;
    return;
  }
  if (session.token && !session.tokenRejected) {
    el.innerHTML = `<div class="unlocked">${I.unlock}<span>Controls unlocked</span><button type="button" class="btn sm ghost" id="lockBtn">Lock</button></div>`;
    $("#lockBtn").addEventListener("click", () => {
      session.token = ""; session.tokenRejected = false; store.set(TOKEN_KEY, "");
      renderAuth(); load();
    });
    return;
  }
  el.innerHTML = tokenFormHtml("", session.tokenRejected);
  wireTokenForms(el);
}
