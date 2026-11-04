// @ts-check
// URL building and fetch wrappers for the dashboard's same-origin /api.
//
// Reads send the token as `?token=` because EventSource cannot set headers;
// mutations send it only as `Authorization: Bearer`, which the server
// requires for them. App names and paths are URL-encoded here.

/** How much of an error response body reaches the UI. */
export const BODY_PREVIEW = 160;

/** @param {string} token */
export const tokenQuery = (token) => (token ? "?token=" + encodeURIComponent(token) : "");

/** @param {string} token */
export const stateUrl = (token) => "/api/state" + tokenQuery(token);

/** @param {string} app @param {string} token */
export const releasesUrl = (app, token) => `/api/apps/${encodeURIComponent(app)}/releases${tokenQuery(token)}`;

/** @param {string} app @param {string} token */
export const logsUrl = (app, token) => `/api/apps/${encodeURIComponent(app)}/logs${tokenQuery(token)}`;

/**
 * @param {string} app
 * @param {string} path action under the app, e.g. "restart" or "services/db/scale"
 */
export const actionUrl = (app, path) => `/api/apps/${encodeURIComponent(app)}/${path}`;

const READ_OPTS = { headers: { Accept: "application/json" }, cache: /** @type {RequestCache} */ ("no-store") };

/**
 * POSTs an app action. Rejects only when the request never got an answer.
 * @param {string} app
 * @param {string} path
 * @param {string} token
 * @param {object} [body] sent as JSON when present
 */
export function postAction(app, path, token, body) {
  return fetch(actionUrl(app, path), {
    method: "POST",
    headers: { Authorization: "Bearer " + token, "Content-Type": "application/json" },
    body: body ? JSON.stringify(body) : undefined,
  });
}

/**
 * @param {string} app
 * @param {string} token
 */
export function fetchReleases(app, token) {
  return fetch(releasesUrl(app, token), READ_OPTS);
}

/** @param {Response} res */
export async function errorBody(res) {
  const txt = await res.text().catch(() => "");
  return (txt || "").trim().slice(0, BODY_PREVIEW);
}

/**
 * `failure` is { status: 0 } when the request never got an answer, and
 * { status, body } when the server answered with an error.
 * @typedef {{ status: number, body?: string }} Failure
 * @typedef {{ locked?: boolean, failure?: Failure, data?: import("./state.js").StateData }} StateResult
 */

/**
 * @param {string} token
 * @returns {Promise<StateResult>}
 */
export async function fetchState(token) {
  let res;
  try {
    res = await fetch(stateUrl(token), READ_OPTS);
  } catch (_) {
    return { failure: { status: 0 } };
  }
  if (res.status === 401) { return { locked: true }; }
  if (!res.ok) { return { failure: { status: res.status, body: await errorBody(res) } }; }
  try {
    return { data: await res.json() };
  } catch (_) {
    return { failure: { status: res.status, body: "response was not JSON" } };
  }
}
