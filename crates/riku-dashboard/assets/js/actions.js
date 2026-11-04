// @ts-check
// Running an app action: the token check, the POST, turning a refusal into
// a toast, the busy state on the clicked button, and the stop confirmation.
import { errorBody, postAction } from "./api.js";
import { renderAuth } from "./auth.js";
import { updateCard } from "./card.js";
import { $ } from "./dom.js";
import { cap, esc } from "./format.js";
import { I } from "./icons.js";
import { domainsOf } from "./render/meta.js";
import { workerName } from "./render/workers.js";
import { session } from "./state.js";
import { toast } from "./toast.js";

/** @typedef {import("./state.js").Card} Card */

/**
 * POSTs `path` for `app` with the control token. Resolves true on success;
 * every failure has already been reported in a toast.
 * @param {string} app
 * @param {string} path
 * @param {object} [body]
 * @returns {Promise<boolean>}
 */
export async function act(app, path, body) {
  if (!session.token) {
    toast("err", "Controls are locked", "Paste the control token to run actions.");
    const i = $("#tokInput");
    if (i) { i.focus(); }
    return false;
  }
  try {
    const res = await postAction(app, path, session.token, body);
    if (res.ok) { return true; }
    if (res.status === 401) {
      session.tokenRejected = true; renderAuth();
      toast("err", "Token rejected", "The supervisor refused the control token (401).");
      return false;
    }
    if (res.status === 403) {
      session.actionsDisabled = true; renderAuth();
      toast("err", `${cap(path)} failed`, "Actions are disabled on this server. Restart the dashboard with --token.");
      return false;
    }
    const txt = await errorBody(res);
    toast("err", `${cap(path)} failed`, `${res.status}${txt ? " · " + txt : ""}`);
  } catch (e) {
    const err = /** @type {Error} */ (e);
    toast("err", `${cap(path)} failed`, (err && err.message) || "Network error");
  }
  return false;
}

/**
 * Shows `label` with a spinner on `btn` while `fn` runs, then restores the
 * button and re-renders the card from its last data.
 * @template T
 * @param {Card} c
 * @param {HTMLButtonElement} btn
 * @param {string} label
 * @param {() => Promise<T>} fn
 * @returns {Promise<T>}
 */
export async function withBusy(c, btn, label, fn) {
  const html = btn.innerHTML;
  btn.style.minWidth = btn.offsetWidth + "px";
  btn.disabled = true;
  btn.setAttribute("aria-busy", "true");
  btn.innerHTML = I.spin + esc(label);
  try { return await fn(); }
  finally {
    btn.removeAttribute("aria-busy");
    btn.style.minWidth = "";
    btn.innerHTML = html;
    btn.disabled = false;
    if (c.data) { updateCard(c, c.data); }
  }
}

/**
 * Asks before stopping an app, falling back to window.confirm where
 * <dialog> is unsupported.
 * @param {Card} c
 * @returns {Promise<boolean>}
 */
export function confirmStop(c) {
  const dlg = /** @type {HTMLDialogElement} */ ($("#confirm"));
  const ws = (c.data && c.data.workers) || [];
  const dom = c.data ? domainsOf(c.data)[0] : "";
  $("#cfTitle").innerHTML = `Stop <span class="mono">${esc(c.app)}</span>?`;
  $("#cfDesc").textContent = ws.length
    ? `${ws.length === 1 ? "This worker" : "These " + ws.length + " workers"} will be shut down${dom ? ", and " + dom + " stops serving traffic" : ""} until the app is started again.`
    : `${c.app} will be stopped.`;
  $("#cfList").innerHTML = ws.map((w) => `<span>${esc(workerName(w))}</span>`).join("");
  $("#cfOk").textContent = "Stop " + c.app;
  if (typeof dlg.showModal !== "function") {
    return Promise.resolve(window.confirm(`Stop ${c.app}? Its workers will be shut down.`));
  }
  return new Promise((resolve) => {
    const onClose = () => { dlg.removeEventListener("close", onClose); resolve(dlg.returnValue === "ok"); };
    dlg.addEventListener("close", onClose);
    dlg.returnValue = "";
    dlg.showModal();
    $("#cfCancel").focus();
  });
}

/** Closes the stop dialog on a backdrop click. */
export function initConfirm() {
  $("#confirm").addEventListener("click", (e) => {
    const dlg = /** @type {HTMLDialogElement} */ (e.currentTarget);
    if (e.target === dlg) { dlg.close("cancel"); }
  });
}
