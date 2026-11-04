// @ts-check
import { $ } from "./dom.js";
import { esc } from "./format.js";
import { I } from "./icons.js";

/** @typedef {{ label: string, run: () => void }} ToastAction */

/**
 * Shows a toast in the corner stack, keeping at most four.
 * @param {"ok" | "err" | ""} kind
 * @param {string} title
 * @param {string} [msg]
 * @param {ToastAction} [action] adds a link button that runs it and dismisses
 */
export function toast(kind, title, msg, action) {
  const box = $("#toasts");
  const t = document.createElement("div");
  t.className = "toast " + (kind || "");
  t.setAttribute("role", kind === "err" ? "alert" : "status");
  t.innerHTML = `<span class="t-ico">${kind === "ok" ? I.check : kind === "err" ? I.bang : I.info}</span>
      <div class="t-body"><span class="t-title">${esc(title)}</span>${msg ? `<span class="t-msg">${esc(msg)}</span>` : ""}${action ? `<button type="button" class="link t-act">${esc(action.label)}</button>` : ""}</div>
      <button type="button" class="t-x" aria-label="Dismiss">${I.x}</button>`;
  /** @type {ReturnType<typeof setTimeout>} */
  let timer;
  const kill = () => { clearTimeout(timer); t.classList.add("out"); setTimeout(() => t.remove(), 180); };
  t.querySelector(".t-x").addEventListener("click", kill);
  if (action) {
    t.querySelector(".t-act").addEventListener("click", () => { action.run(); kill(); });
  }
  t.addEventListener("mouseenter", () => clearTimeout(timer));
  t.addEventListener("mouseleave", () => { timer = setTimeout(kill, 2500); });
  box.appendChild(t);
  while (box.children.length > 4) { box.firstElementChild.remove(); }
  timer = setTimeout(kill, kind === "err" ? 8000 : 5000);
}
