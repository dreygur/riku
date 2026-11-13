// @ts-check
// The static skeleton of one app card. updateCard() in card.js fills the
// data-* slots on every poll.
import { esc } from "../format.js";
import { I } from "../icons.js";

/** Upper bound for the web and service scale steppers. */
export const MAX_SCALE = 32;

/**
 * @param {string} id DOM id prefix from nextCardId()
 * @param {string} app app name from the server
 */
export function cardHtml(id, app) {
  return `
      <div class="progress" aria-hidden="true"><i></i></div>
      <div class="app-head">
        <div class="app-id">
          <div class="app-title"><h2 id="${id}">${esc(app)}</h2><span data-health></span></div>
          <div class="meta" data-meta></div>
        </div>
        <div class="actions">
          <button type="button" class="btn" data-do="logs">${I.term}Logs</button>
          <button type="button" class="btn" data-do="restart">${I.restart}Restart</button>
          <button type="button" class="btn primary" data-do="redeploy">${I.deploy}Redeploy</button>
        </div>
      </div>
      <div class="alert" data-alert hidden></div>
      <div class="table-wrap">
        <div class="table" role="table" aria-label="${esc(app)} workers">
          <div class="tr th" role="row">
            <span role="columnheader">Worker</span><span role="columnheader">State</span><span role="columnheader">Memory</span>
            <span role="columnheader">CPU</span><span role="columnheader">PID / ID</span><span role="columnheader" class="r">Restarts</span>
          </div>
          <div role="rowgroup" data-rows></div>
        </div>
      </div>
      <div class="app-foot">
        <div class="controls" data-controls>
          <div class="ctl" data-scale-ctl hidden>
            <span class="ctl" data-kinds></span>
            <button type="button" class="btn" data-do="scale" disabled>Apply</button>
            <button type="button" class="btn ghost" data-do="reset" hidden>Reset</button>
          </div>
          <span class="vsep" aria-hidden="true" data-scale-sep hidden></span>
          ${releaseControlHtml(id)}
          <span class="grow"></span>
          <button type="button" class="btn danger" data-do="stop">${I.stop}Stop app</button>
        </div>
        <div class="busy-note" data-busynote hidden>
          ${I.spin}
          <p>Deploy in progress. Scaling, rollback and stop unlock when it finishes.</p>
          <span class="grow"></span>
          <button type="button" class="link" data-do="logs">Follow deploy output ${I.arrow}</button>
        </div>
      </div>`;
}

/**
 * One stepper per process kind the scale API takes (`process_kinds` in
 * /api/state), so a worker only app shows no web stepper.
 * @param {string} id DOM id prefix of the card
 * @param {string[]} kinds process kinds from the server
 */
export function kindSteppersHtml(id, kinds) {
  return kinds.map((kind, i) => {
    const k = esc(kind);
    return `<label class="ctl-label" for="${id}-scale-${i}">${k} processes</label>
            <div class="stepper" data-stepper="${k}">
              <button type="button" class="st-btn" data-step="-1" data-kind="${k}" aria-label="Fewer ${k} processes">${I.minus}</button>
              <input id="${id}-scale-${i}" type="number" min="0" max="${MAX_SCALE}" inputmode="numeric" data-scale="${k}" />
              <button type="button" class="st-btn" data-step="1" data-kind="${k}" aria-label="More ${k} processes">${I.plus}</button>
            </div>`;
  }).join("");
}

/**
 * The release select and rollback button, shared by process and container
 * cards. renderReleases() in releases.js fills the options.
 * @param {string} id DOM id prefix of the card
 */
export function releaseControlHtml(id) {
  return `<div class="ctl">
            <label class="ctl-label" for="${id}-rel">Release</label>
            <div class="select"><select id="${id}-rel" data-rel><option value="">loading...</option></select>${I.chev}</div>
            <button type="button" class="btn" data-do="rollback">${I.rollback}Roll back</button>
          </div>`;
}
