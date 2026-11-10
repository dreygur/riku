// @ts-check
// Markup for the whole-page states: loading skeleton, nothing deployed, and
// locked, plus the token form both the header and the locked panel use.
import { esc } from "../format.js";
import { I } from "../icons.js";

const LOOPBACK_HOSTS = ["localhost", "127.0.0.1", "::1", "[::1]"];

/**
 * @param {string} idSuffix keeps the input id unique per form
 * @param {boolean} rejected the last token got a 401
 */
export function tokenFormHtml(idSuffix, rejected) {
  return `<form class="tok ${rejected ? "bad" : ""}" data-tok>
      <label class="field">${I.lock}<span class="sr">Control token</span>
        <input id="tokInput${idSuffix}" type="password" placeholder="Paste control token" autocomplete="off" spellcheck="false" ${rejected ? 'aria-invalid="true" aria-describedby="tokErr' + idSuffix + '"' : ""} />
      </label>
      <button type="submit" class="btn primary">Unlock</button>
    </form>
    ${rejected ? `<span class="tok-err" id="tokErr${idSuffix}">Token rejected (401)</span>` : ""}`;
}

export function skeletonHtml() {
  const card = `<div class="app" aria-hidden="true">
      <div class="sk-head"><span class="sk" style="width:140px;height:16px"></span><span class="sk lo" style="width:80px;height:16px"></span><span class="grow"></span><span class="sk lo" style="width:72px;height:32px"></span><span class="sk lo" style="width:96px;height:32px"></span></div>
      <div class="sk-row"><span class="sk" style="height:10px"></span><span class="sk lo" style="height:10px"></span><span class="sk lo" style="height:6px"></span></div>
      <div class="sk-row"><span class="sk" style="height:10px"></span><span class="sk lo" style="height:10px"></span><span class="sk lo" style="height:6px"></span></div>
    </div>`;
  return `<p class="sr" role="status">Reading the supervisor...</p>${card}${card}`;
}

/**
 * Over an ssh tunnel the page sees a loopback name, which is not where a
 * laptop would push to.
 * @param {string} host the page's location.hostname
 */
export function pushHost(host) {
  return !host || LOOPBACK_HOSTS.includes(host) ? "<host>" : host;
}

/** @param {string} hostname the page's location.hostname */
export function emptyHtml(hostname) {
  const host = pushHost(hostname);
  const cmds = [`rk server add <name> riku@${host} --admin <login>@${host}`, "git push riku main"];
  return `<div class="panel">
      <div><h2>Nothing deployed yet</h2><p>riku picks up an app as soon as you push it.</p></div>
      <div class="cmds">${cmds.map((c, i) => `<div class="cmd"><span class="n">${i + 1}</span><code${i ? ' class="c-ok"' : ""}>${esc(c)}</code><button type="button" data-copy="${esc(c)}">Copy</button></div>`).join("")}</div>
    </div>`;
}

/** @param {boolean} rejected the last token got a 401 */
export function lockedHtml(rejected) {
  return `<div class="panel">
      <span class="panel-ico">${I.lockLg}</span>
      <div><h2>This supervisor is locked</h2><p>Enter the control token to see apps and run actions. It is kept in this browser only.</p></div>
      ${tokenFormHtml("Panel", rejected)}
    </div>`;
}
