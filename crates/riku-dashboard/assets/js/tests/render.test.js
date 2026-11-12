import { test } from "node:test";
import assert from "node:assert/strict";

import { esc, uptimeText } from "../format.js";
import { serviceControlsHtml } from "../render/containers.js";
import { alertHtml, healthBadge, lockedControls, routeHtml } from "../render/meta.js";
import { emptyHtml, pushHost } from "../render/panels.js";
import { stateOf, workerRow } from "../render/workers.js";

const EVIL = `"><img src=x onerror=alert(1)>'`;

/** Fails with the offending markup when raw attacker text survives. */
function assertNoRawMarkup(html) {
  assert.ok(!html.includes("<img"), `unescaped tag in: ${html}`);
  assert.ok(!html.includes(`onerror=alert(1)>'`), `attribute breakout in: ${html}`);
}

test("esc covers every character that can open a tag or close an attribute", () => {
  assert.equal(esc(`&<>"'`), "&amp;&lt;&gt;&quot;&#39;");
  assert.equal(esc(null), "");
});

test("worker rows escape the worker name and container image, text and title alike", () => {
  const html = workerRow({ kind: EVIL, ordinal: 1, unit: "container", image: EVIL, state_detail: EVIL, container_id: EVIL }, 0);
  assertNoRawMarkup(html);
  assert.ok(html.includes(esc(EVIL)));
});

test("the memory bar measures against the worker's limit, and against the app peak without one", () => {
  const mib = 1024 * 1024;
  const limited = workerRow({ kind: "web", ordinal: 1, memory_bytes: 64 * mib, memory_limit_bytes: 128 * mib }, 64 * mib);
  assert.ok(limited.includes('aria-label="50% of memory limit"'), limited);
  const open = workerRow({ kind: "web", ordinal: 1, memory_bytes: 64 * mib, memory_limit_bytes: null }, 64 * mib);
  assert.ok(open.includes('aria-label="memory"'), open);
});

test("an unknown router's error is escaped inside the title attribute", () => {
  assertNoRawMarkup(routeHtml({ router: "?", error: EVIL }));
  assertNoRawMarkup(routeHtml({ router: "plugin", name: EVIL }));
});

test("a caddy site the deploy user cannot see reads as unknown, not off", () => {
  assert.ok(routeHtml({ router: "caddy", site: null, site_known: false }).includes("unknown"));
  assert.ok(routeHtml({ router: "caddy", site: null, site_known: true }).includes("off"));
  assert.ok(routeHtml({ router: "nginx", config_exists: false, config_known: false, enabled: false }).includes("unknown"));
  assert.ok(routeHtml({ router: "nginx", config_exists: false, config_known: true, enabled: false }).includes("off"));
});

test("an app no deploy released reads as never deployed, with only logs and redeploy open", () => {
  const badge = healthBadge(false, [], true);
  assert.ok(badge.includes("Never deployed"), badge);
  assert.deepEqual(lockedControls(false, [], true), { restart: true, redeploy: false, stop: true, scale: true });
});

test("a stopped app can restart or redeploy, but not stop or scale", () => {
  const off = [stateOf("stopped")];
  assert.ok(healthBadge(false, off, false).includes("Stopped"));
  assert.deepEqual(lockedControls(false, off, false), { restart: false, redeploy: false, stop: true, scale: true });
  const crashed = [stateOf("crashed")];
  assert.equal(lockedControls(false, crashed, false).stop, false, "a crashing worker can still be stopped");
});

test("a running app has every control open until a deploy holds the lock", () => {
  const live = [stateOf("running")];
  assert.deepEqual(lockedControls(false, live, false), { restart: false, redeploy: false, stop: false, scale: false });
  assert.deepEqual(lockedControls(true, live, false), { restart: true, redeploy: true, stop: true, scale: true });
});

test("the crash alert escapes the worker name in its data-worker attribute", () => {
  const ws = [{ kind: EVIL, ordinal: 1, status: "crashed" }];
  assertNoRawMarkup(alertHtml(ws, ws.map((w) => stateOf(w.status)), 1));
});

test("service controls escape the service name everywhere it appears", () => {
  assertNoRawMarkup(serviceControlsHtml("app-1", [{ kind: EVIL, ordinal: 1, unit: "container" }], {}));
});

test("the empty panel escapes the host in the copy commands, and hides loopback names", () => {
  assertNoRawMarkup(emptyHtml(EVIL));
  assert.equal(pushHost("127.0.0.1"), "<host>");
  assert.equal(pushHost("deploy.example.com"), "deploy.example.com");
});

test("the supervisor pill shows why the uptime is missing when the server says", () => {
  const why = "not read as root, the pid file belongs to the deploy user";
  assert.equal(uptimeText({ supervisor_uptime_seconds: null, supervisor_uptime_unavailable: why }), why);
  assert.equal(uptimeText({ supervisor_uptime_seconds: null }), "uptime unknown");
  assert.equal(uptimeText({ supervisor_uptime_seconds: 3700 }), "up 1h 1m");
});
