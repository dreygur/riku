import { test, afterEach } from "node:test";
import assert from "node:assert/strict";

import { actionUrl, fetchState, logsUrl, releasesUrl, stateUrl } from "../api.js";

const realFetch = globalThis.fetch;
afterEach(() => { globalThis.fetch = realFetch; });

test("read urls carry the token url-encoded, and none when there is no token", () => {
  assert.equal(stateUrl(""), "/api/state");
  assert.equal(stateUrl("a b&c"), "/api/state?token=a%20b%26c");
});

test("app names are encoded so they cannot change the request path", () => {
  assert.equal(releasesUrl("../x?y", "t"), "/api/apps/..%2Fx%3Fy/releases?token=t");
  assert.equal(logsUrl("my app", ""), "/api/apps/my%20app/logs");
});

test("action urls never put the token in the query", () => {
  assert.equal(actionUrl("web/1", "services/db/scale"), "/api/apps/web%2F1/services/db/scale");
});

test("fetchState maps 401, a network error and a non-JSON body to distinct results", async () => {
  globalThis.fetch = async () => new Response("no", { status: 401 });
  assert.deepEqual(await fetchState("t"), { locked: true });

  globalThis.fetch = async () => { throw new TypeError("offline"); };
  assert.deepEqual(await fetchState("t"), { failure: { status: 0 } });

  globalThis.fetch = async () => new Response("<html>", { status: 200 });
  assert.deepEqual(await fetchState("t"), { failure: { status: 200, body: "response was not JSON" } });
});

test("fetchState trims and truncates an error body to 160 characters", async () => {
  globalThis.fetch = async () => new Response("  " + "x".repeat(300) + "  ", { status: 500 });
  const got = await fetchState("");
  assert.equal(got.failure.status, 500);
  assert.equal(got.failure.body, "x".repeat(160));
});
