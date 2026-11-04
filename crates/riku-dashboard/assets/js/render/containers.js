// @ts-check
// Controls for an app run as container services: one stepper and action row
// per service, then the shared release and stop controls.
import { esc } from "../format.js";
import { I } from "../icons.js";
import { MAX_SCALE, releaseControlHtml } from "./apps.js";

/**
 * @param {string} id DOM id prefix of the card
 * @param {import("../state.js").Worker[]} ws the app's workers
 * @param {Record<string, number>} pending copies typed but not applied, per service
 */
export function serviceControlsHtml(id, ws, pending) {
  const services = [...new Set(ws.map((w) => w.kind))];
  const svcsHtml = services.map((svc) => {
    const count = ws.filter((w) => w.kind === svc).length;
    const curScale = pending && pending[svc] != null ? pending[svc] : count;
    const dirty = curScale !== count;
    return `<div class="ctl" data-service-group="${esc(svc)}">
          <label class="ctl-label">Service <b>${esc(svc)}</b></label>
          <div class="stepper" data-svc-stepper="${esc(svc)}">
            <button type="button" class="st-btn" data-svc-step="-1" data-service="${esc(svc)}" aria-label="Fewer ${esc(svc)} copies">${I.minus}</button>
            <input type="number" min="0" max="${MAX_SCALE}" inputmode="numeric" data-svc-scale="${esc(svc)}" value="${curScale}" />
            <button type="button" class="st-btn" data-svc-step="1" data-service="${esc(svc)}" aria-label="More ${esc(svc)} copies">${I.plus}</button>
          </div>
          <button type="button" class="btn sm ${dirty ? "primary" : ""}" data-do="svc-scale" data-service="${esc(svc)}" ${dirty ? "" : "disabled"}>Apply</button>
          <button type="button" class="btn sm" data-do="svc-restart" data-service="${esc(svc)}">${I.restart}Restart</button>
          <button type="button" class="btn sm danger" data-do="svc-stop" data-service="${esc(svc)}">${I.stop}Stop</button>
          <button type="button" class="btn sm" data-do="logs" data-worker="${esc(svc)}">${I.term}Logs</button>
        </div>`;
  }).join('<span class="vsep" aria-hidden="true"></span>');

  return svcsHtml + `<span class="vsep" aria-hidden="true"></span>
        ${releaseControlHtml(id)}
        <span class="grow"></span>
        <button type="button" class="btn danger" data-do="stop">${I.stop}Stop app</button>`;
}
