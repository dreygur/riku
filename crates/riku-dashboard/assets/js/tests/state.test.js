import { test } from "node:test";
import assert from "node:assert/strict";

import { nextCardId, store, tokenFromUrl } from "../state.js";

test("a url token wins over the stored one and is stripped from the url", () => {
  const got = tokenFromUrl("http://localhost:8088/?a=1&token=%20secret%20&b=2#x", "old");
  assert.deepEqual(got, { token: "secret", cleanPath: "/?a=1&b=2#x" });
});

test("without a url token the stored token is used and the url is left alone", () => {
  assert.deepEqual(tokenFromUrl("http://localhost:8088/", "stored"), { token: "stored", cleanPath: null });
});

test("an empty token parameter still gets removed from the url", () => {
  assert.deepEqual(tokenFromUrl("http://localhost:8088/?token=", ""), { token: "", cleanPath: "/" });
});

test("card ids stay unique for names that would sanitize alike", () => {
  assert.notEqual(nextCardId(), nextCardId());
});

test("store reads as empty when localStorage is unavailable", () => {
  // Node runs these tests without web storage, which is the blocked-storage case.
  assert.equal(store.get("riku_token"), "");
  assert.doesNotThrow(() => store.set("riku_token", "x"));
});
