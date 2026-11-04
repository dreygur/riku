// @ts-check
// Entry point: adopts a `?token=` from the URL, wires the page-level
// listeners, and starts the refresh loop.
import { initConfirm } from "./actions.js";
import { renderAuth } from "./auth.js";
import { $ } from "./dom.js";
import { initDrawer } from "./logs/drawer.js";
import { initCopyButtons } from "./page.js";
import { initPoll, load } from "./poll.js";
import { skeletonHtml } from "./render/panels.js";
import { TOKEN_KEY, session, store, tokenFromUrl } from "./state.js";

/**
 * A token in the URL is saved and then dropped from the address bar, so it
 * does not linger in history or get copied along with the link.
 */
function adoptUrlToken() {
  const { token, cleanPath } = tokenFromUrl(location.href, store.get(TOKEN_KEY));
  session.token = token;
  if (cleanPath == null) { return; }
  if (token) { store.set(TOKEN_KEY, token); }
  try { history.replaceState(null, "", cleanPath); } catch (_) {}
}

adoptUrlToken();
initConfirm();
initCopyButtons();
initPoll();
initDrawer();

$("#apps").innerHTML = skeletonHtml();
renderAuth();
load();
