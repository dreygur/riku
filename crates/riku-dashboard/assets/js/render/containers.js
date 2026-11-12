// @ts-check
// Controls for an app run as container services: a grid with one stepper
// and action row per service, then a row of its own for the release and
// stop app controls, laid out like a Procfile app's footer.
import { esc } from "../format.js";
import { I } from "../icons.js";
import { MAX_SCALE, releaseControlHtml } from "./apps.js";

/**
 * @param {string} svc service name from the server
 * @param {number} count copies running now
 * @param {number} curScale copies shown in the stepper
 */
function serviceRowHtml(svc, count, curScale) {
  const s = esc(svc);
  const dirty = curScale !== count;
  return `<div class="svc-row" data-service-group="${s}">
          <span class="ctl-label svc-name" title="${s}">Service <b>${s}</b></span>
          <div class="stepper" data-svc-stepper="${s}">
            <button type="button" class="st-btn" data-svc-step="-1" data-service="${s}" aria-label="Fewer ${s} copies">${I.minus}</button>
            <input type="number" min="0" max="${MAX_SCALE}" inputmode="numeric" data-svc-scale="${s}" value="${curScale}" aria-label="${s} copies" />
            <button type="button" class="st-btn" data-svc-step="1" data-service="${s}" aria-label="More ${s} copies">${I.plus}</button>
          </div>
          <button type="button" class="btn sm ${dirty ? "primary" : ""}" data-do="svc-scale" data-service="${s}" ${dirty ? "" : "disabled"}>Apply</button>
          <button type="button" class="btn sm" data-do="svc-restart" data-service="${s}">${I.restart}Restart</button>
          <button type="button" class="btn sm danger" data-do="svc-stop" data-service="${s}">${I.stop}Stop</button>
          <button type="button" class="btn sm" data-do="logs" data-worker="${s}">${I.term}Logs</button>
        </div>`;
}

/**
 * @param {string} id DOM id prefix of the card
 * @param {import("../state.js").Worker[]} ws the app's workers
 * @param {Record<string, number>} pending copies typed but not applied, per service
 */
export function serviceControlsHtml(id, ws, pending) {
  const services = [...new Set(ws.map((w) => w.kind))];
  const rows = services.map((svc) => {
    const count = ws.filter((w) => w.kind === svc).length;
    const curScale = pending && pending[svc] != null ? pending[svc] : count;
    return serviceRowHtml(svc, count, curScale);
  }).join("");

  return `<div class="svc-grid">${rows}</div>
        <div class="app-row">
          ${releaseControlHtml(id)}
          <span class="grow"></span>
          <button type="button" class="btn danger" data-do="stop">${I.stop}Stop app</button>
        </div>`;
}
