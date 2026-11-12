// @ts-check
// Page-wide state shared across modules, plus the shapes /api/state returns.
//
// The control token lives in `session.token` and in localStorage under
// TOKEN_KEY. It only ever leaves the page in the Authorization header or the
// `?token=` query of a same-origin /api request.

/**
 * @typedef {object} Worker
 * @property {string} kind
 * @property {number} ordinal
 * @property {string} [status]
 * @property {number} [pid]
 * @property {number} [memory_bytes]
 * @property {number | null} [memory_limit_bytes] cgroup memory.max or the configured limit, null when none
 * @property {number} [cpu_time_ms]
 * @property {number} [restart_count]
 * @property {string} [unit] "container" for container services
 * @property {string} [container_id]
 * @property {number} [cpu_percent]
 * @property {string} [state_detail]
 * @property {string} [image]
 */

/**
 * Tagged by `router`: nginx carries enabled/config_exists and config_known
 * (false when the user cannot read /etc/riku/nginx), caddy the site
 * file path (null without one) and site_known (false when the deploy user
 * cannot read the site root, so a null site means nothing), plugin its name,
 * and unknown the error from reading the ROUTER file.
 * @typedef {object} Route
 * @property {string} [router]
 * @property {boolean} [enabled]
 * @property {boolean} [config_exists]
 * @property {boolean} [config_known]
 * @property {string | null} [site]
 * @property {boolean} [site_known]
 * @property {string} [name]
 * @property {string} [error]
 */

/**
 * A url the app is live at. label is "domain" for one of its domains, "lan"
 * for a port or path url on an address the internet cannot reach, "public"
 * for one on a public address.
 * @typedef {object} LiveUrl
 * @property {string} url
 * @property {string} label
 */

/**
 * A compose service of a container app.
 * @typedef {object} Service
 * @property {string} name
 * @property {boolean} built riku builds its image from the app's code
 * @property {string | null} image the image a pulled service runs, null for a built one
 */

/**
 * @typedef {object} App
 * @property {string} app
 * @property {Service[]} [services] compose services in start order, empty for a Procfile app
 * @property {Worker[]} [workers]
 * @property {Record<string, string>} [routing]
 * @property {Route} [route]
 * @property {LiveUrl[]} [urls] where the app is live now, from its last route apply, as a deploy prints it; empty without a route
 * @property {LiveUrl[]} [pending_urls] what the current settings give when that differs from urls; live after a redeploy
 * @property {string} [deploy_lock] "held" while a deploy runs
 * @property {boolean} [released] false while no deploy has recorded a release
 * @property {string[]} [process_kinds] kinds the scale API takes, empty for a container app
 */

/**
 * @typedef {object} StateData
 * @property {App[]} [apps]
 * @property {number | null} [supervisor_uptime_seconds]
 * @property {string} [supervisor_uptime_unavailable] why the uptime was not read (a root run)
 * @property {{ name: string, version: string }} [engine]
 * @property {string} [riku_version]
 */

/** @typedef {{ ts: number, sha: string }} Release */

/**
 * One rendered app card and what it last showed.
 * @typedef {object} Card
 * @property {string} app
 * @property {string} id DOM id prefix, unique per card
 * @property {HTMLElement} el
 * @property {App | null} data last /api/state entry for the app
 * @property {Release[]} releases newest first
 * @property {Map<string, number>} kindNow copies of each process kind now
 * @property {Map<string, number>} kindScale pending counts, only for kinds that differ from now
 * @property {string} kindsKey the process kinds the steppers were drawn for
 * @property {boolean} scaleLocked scaling is off: a deploy runs, or no worker is up
 * @property {boolean} wasBusy
 * @property {Record<string, number>} svcScale pending copies per container service
 */

/** @typedef {"loading" | "apps" | "empty" | "locked"} Mode */

export const TOKEN_KEY = "riku_token";

/** localStorage that reads as empty and ignores writes when storage is blocked. */
export const store = {
  /** @param {string} k */
  get(k) { try { return localStorage.getItem(k) || ""; } catch (_) { return ""; } },
  /** @param {string} k @param {string} v empty removes the key */
  set(k, v) { try { v ? localStorage.setItem(k, v) : localStorage.removeItem(k); } catch (_) {} },
};

export const session = {
  token: "",
  tokenRejected: false,
  /**
   * Set when an action gets 403: the server runs without --token, so no
   * token can enable mutations.
   */
  actionsDisabled: false,
  /** @type {Mode} */
  mode: "loading",
};

/**
 * Picks the token from a `?token=` query, falling back to `stored`.
 * `cleanPath` is the same URL without the token, or null when the URL had
 * no token parameter and needs no rewrite.
 * @param {string} href
 * @param {string} stored
 * @returns {{ token: string, cleanPath: string | null }}
 */
export function tokenFromUrl(href, stored) {
  const url = new URL(href);
  const token = (url.searchParams.get("token") || stored || "").trim();
  if (!url.searchParams.has("token")) {
    return { token, cleanPath: null };
  }
  url.searchParams.delete("token");
  return { token, cleanPath: url.pathname + url.search + url.hash };
}

/** @type {Map<string, Card>} */
export const cards = new Map();

let cardSeq = 0;

/**
 * A counter, not the app name, keeps ids unique: my.app and my_app would
 * sanitize to the same id.
 */
export function nextCardId() {
  cardSeq++;
  return "app-" + cardSeq;
}
