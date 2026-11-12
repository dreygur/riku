// @ts-check
// The rows of an app run as container services: a header row per service
// (name, image, stepper and actions) with its copies under it, and the
// footer with the release and stop app controls.
import { esc } from "../format.js";
import { I } from "../icons.js";
import { MAX_SCALE, releaseControlHtml } from "./apps.js";
import { imageLabel, workerRow } from "./workers.js";

/**
 * The image label of a service: what its copies run, else what the state
 * says it runs.
 * @param {import("../state.js").Service} svc
 * @param {import("../state.js").Worker[]} copies
 * @returns {{ text: string, title: string } | null}
 */
function serviceImage(svc, copies) {
  const running = copies.find((w) => w.image);
  if (running) { return imageLabel(String(running.image)); }
  if (svc.built) { return { text: "built", title: "built by riku" }; }
  return svc.image ? imageLabel(svc.image) : null;
}

/**
 * @param {string} name service name from the server
 * @param {number} curScale copies shown in the stepper
 * @param {boolean} dirty the stepper differs from the copies running
 */
function serviceControlsHtml(name, curScale, dirty) {
  const s = esc(name);
  return `<span role="cell" class="svc-ctl">
          <span class="stepper" data-svc-stepper="${s}">
            <button type="button" class="st-btn" data-svc-step="-1" data-service="${s}" aria-label="Fewer ${s} copies">${I.minus}</button>
            <input type="number" min="0" max="${MAX_SCALE}" inputmode="numeric" data-svc-scale="${s}" value="${curScale}" aria-label="${s} copies" />
            <button type="button" class="st-btn" data-svc-step="1" data-service="${s}" aria-label="More ${s} copies">${I.plus}</button>
          </span>
          <button type="button" class="btn sm ${dirty ? "primary" : ""}" data-do="svc-scale" data-service="${s}" ${dirty ? "" : "disabled"}>Apply</button>
          <button type="button" class="btn sm" data-do="svc-restart" data-service="${s}">${I.restart}Restart</button>
          <button type="button" class="btn sm danger" data-do="svc-stop" data-service="${s}">${I.stop}Stop</button>
          <button type="button" class="btn sm" data-do="logs" data-worker="${s}">${I.term}Logs</button>
        </span>`;
}

/**
 * @param {import("../state.js").Service} svc
 * @param {import("../state.js").Worker[]} copies the service's copies
 * @param {Record<string, number>} pending copies typed but not applied, per service
 */
function serviceHeadHtml(svc, copies, pending) {
  const name = esc(svc.name);
  const img = serviceImage(svc, copies);
  const imgHtml = img ? `<span class="w-img" title="${esc(img.title)}">${esc(img.text)}</span>` : "";
  const curScale = pending && pending[svc.name] != null ? pending[svc.name] : copies.length;
  return `<div class="tr svc-head" role="row" data-service-head="${name}">
      <span role="cell" class="w-name"><span class="w-label">${name}</span>${imgHtml}</span>
      ${serviceControlsHtml(svc.name, curScale, curScale !== copies.length)}
    </div>`;
}

/**
 * A service known only from the kind of its copies, so nothing about its
 * image.
 * @param {string} name
 * @returns {import("../state.js").Service}
 */
function kindService(name) {
  return { name, built: false, image: null };
}

/**
 * Every service in start order, each followed by its copies. Without a
 * service list from the server the services are the copies' kinds.
 * @param {import("../state.js").Service[]} services compose services from /api/state
 * @param {import("../state.js").Worker[]} ws the app's workers, sorted
 * @param {Record<string, number>} pending copies typed but not applied, per service
 */
export function containerRowsHtml(services, ws, pending) {
  const list = services.length
    ? services
    : [...new Set(ws.map((w) => w.kind))].map(kindService);
  return list.map((svc) => {
    const copies = ws.filter((w) => w.kind === svc.name);
    return serviceHeadHtml(svc, copies, pending) + copies.map((w) => workerRow(w)).join("");
  }).join("");
}

/**
 * The app level controls of a container app, laid out like a Procfile
 * app's footer.
 * @param {string} id DOM id prefix of the card
 */
export function containerFooterHtml(id) {
  return `${releaseControlHtml(id)}
        <span class="grow"></span>
        <button type="button" class="btn danger" data-do="stop">${I.stop}Stop app</button>`;
}
