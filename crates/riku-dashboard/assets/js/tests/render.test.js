import { test } from "node:test";
import assert from "node:assert/strict";

import { esc } from "../format.js";
import { serviceControlsHtml } from "../render/containers.js";
import { alertHtml, routeHtml } from "../render/meta.js";
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

test("an unknown router's error is escaped inside the title attribute", () => {
  assertNoRawMarkup(routeHtml({ router: "?", error: EVIL }));
  assertNoRawMarkup(routeHtml({ router: "plugin", name: EVIL }));
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
