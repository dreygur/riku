import { test } from "node:test";
import assert from "node:assert/strict";

import { esc, uptimeText } from "../format.js";
import { kindSteppersHtml } from "../render/apps.js";
import { serviceControlsHtml } from "../render/containers.js";
import { alertHtml, healthBadge, lockedControls, metaHtml, routeHtml } from "../render/meta.js";
import { emptyHtml, pushHost } from "../render/panels.js";
import { imageLabel, stateOf, workerRow } from "../render/workers.js";

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

test("the memory bar measures against the worker's limit, and there is no bar without one", () => {
  const mib = 1024 * 1024;
  const limited = workerRow({ kind: "web", ordinal: 1, memory_bytes: 64 * mib, memory_limit_bytes: 128 * mib });
  assert.ok(limited.includes('aria-label="50% of memory limit"'), limited);
  for (const unit of ["container", undefined]) {
    const open = workerRow({ kind: "web", ordinal: 1, unit, memory_bytes: 64 * mib, memory_limit_bytes: null });
    assert.ok(!open.includes('class="meter"'), `a bar without a limit: ${open}`);
    assert.ok(open.includes("64") && open.includes("no limit"), open);
  }
});

test("a container row keeps the name on one line and shows the image as a short label", () => {
  const pulled = workerRow({ kind: "redis", ordinal: 1, unit: "container", image: "redis:7-alpine" });
  assert.ok(pulled.includes('<span class="w-img" title="redis:7-alpine">redis:7-alpine</span>'), pulled);
  const built = workerRow({ kind: "web", ordinal: 2, unit: "container", image: "localhost/riku-0b00b8f683ba20d5:latest" });
  assert.ok(built.includes(">built</span>"), built);
  assert.ok(built.includes('title="built by riku: localhost/riku-0b00b8f683ba20d5:latest"'), built);
  assert.ok(!built.includes(">localhost/riku-"), "the built tag is shown as text: " + built);
  assert.equal(imageLabel("localhost/riku-0b00b8f683ba20d5").text, "built");
  assert.equal(imageLabel("ghcr.io/acme/localhost/riku-x:1").text, "ghcr.io/acme/localhost/riku-x:1");
});

test("a worker only app gets a worker stepper and no web one, kind names escaped", () => {
  const html = kindSteppersHtml("app-1", ["worker"]);
  assert.ok(html.includes("worker processes") && html.includes('data-scale="worker"'), html);
  assert.ok(!html.toLowerCase().includes("web"), html);
  assertNoRawMarkup(kindSteppersHtml("app-1", [EVIL]));
});

test("an unknown router's error is escaped inside the title attribute", () => {
  assertNoRawMarkup(routeHtml({ router: "?", error: EVIL }));
  assertNoRawMarkup(routeHtml({ router: "plugin", name: EVIL }));
});

test("a router site the deploy user cannot see shows the router alone, the reason in a tooltip, not off", () => {
  const caddy = routeHtml({ router: "caddy", site: null, site_known: false });
  assert.ok(caddy.includes("caddy") && !caddy.includes(">unknown<") && !caddy.includes(">off<"), caddy);
  assert.ok(/title="[^"]*unknown[^"]*"/.test(caddy), caddy);
  assert.ok(routeHtml({ router: "caddy", site: null, site_known: true }).includes("off"));
  const nginx = routeHtml({ router: "nginx", config_exists: false, config_known: false, enabled: false });
  assert.ok(!nginx.includes(">unknown<") && /title="[^"]*unknown[^"]*"/.test(nginx), nginx);
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

test("the card links the app's urls with their label, and says no domain only without one", () => {
  const portMode = metaHtml({ app: "qs-py", routing: { RIKU_NGINX_PORTMAP: "true" }, urls: [{ url: "http://10.0.0.5:8102", label: "lan" }] }, undefined, false);
  assert.ok(portMode.includes('href="http://10.0.0.5:8102"') && portMode.includes("LAN"), portMode);
  assert.ok(!portMode.includes("no domain"), portMode);

  const workerOnly = metaHtml({ app: "jobs", routing: { RIKU_DOMAINS: "jobs.example" }, urls: [] }, undefined, false);
  assert.ok(workerOnly.includes("no domain") && !workerOnly.includes("href="), workerOnly);
});

test("a url the settings give but the route does not yet is a note to redeploy, not the link", () => {
  const changed = metaHtml({
    app: "clacktest",
    urls: [{ url: "http://10.0.0.5:8100", label: "lan" }],
    pending_urls: [{ url: "http://clacktest.10.0.0.5.example", label: "domain" }],
  }, undefined, false);
  assert.ok(changed.includes('href="http://10.0.0.5:8100"'), changed);
  assert.ok(changed.includes("settings changed, redeploy to use http://clacktest.10.0.0.5.example"), changed);
  assert.ok(!changed.includes('href="http://clacktest'), "the pending url is not linked: " + changed);

  const same = metaHtml({ app: "qs-py", urls: [{ url: "http://10.0.0.5:8102", label: "lan" }], pending_urls: [] }, undefined, false);
  assert.ok(!same.includes("settings changed"), same);

  const evil = metaHtml({ app: "a", urls: [], pending_urls: [{ url: `http://x/${EVIL}`, label: "domain" }] }, undefined, false);
  assertNoRawMarkup(evil);
});

test("a url is escaped, and one that is not http or https is not linked", () => {
  const evil = metaHtml({ app: "a", urls: [{ url: `http://x/${EVIL}`, label: "domain" }] }, undefined, false);
  assertNoRawMarkup(evil);
  const script = metaHtml({ app: "a", urls: [{ url: "javascript:alert(1)", label: "domain" }] }, undefined, false);
  assert.ok(!script.includes("href="), script);
});

test("the crash alert escapes the worker name in its data-worker attribute", () => {
  const ws = [{ kind: EVIL, ordinal: 1, status: "crashed" }];
  assertNoRawMarkup(alertHtml(ws, ws.map((w) => stateOf(w.status)), 1));
});

test("service controls escape the service name everywhere it appears", () => {
  assertNoRawMarkup(serviceControlsHtml("app-1", [{ kind: EVIL, ordinal: 1, unit: "container" }], {}));
});

test("service rows sit in one grid with no divider, and the app controls get their own row", () => {
  const ws = ["redis", "web", "worker"].map((kind) => ({ kind, ordinal: 1, unit: "container" }));
  const html = serviceControlsHtml("app-1", ws, {});
  assert.ok(!html.includes("vsep"), "a divider trails a service row: " + html);
  const grid = html.indexOf('class="svc-grid"');
  const appRow = html.indexOf('class="app-row"');
  assert.ok(grid >= 0 && appRow > grid, html);
  assert.equal((html.match(/class="svc-row"/g) || []).length, 3, html);
  const lastService = html.lastIndexOf('data-do="svc-stop"');
  for (const appControl of ["data-rel", 'data-do="rollback"', 'data-do="stop"']) {
    assert.ok(html.indexOf(appControl) > appRow && html.indexOf(appControl) > lastService, `${appControl} is not in the app row: ${html}`);
  }
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
