import { test } from "node:test";
import assert from "node:assert/strict";

import { setScale, valueOf } from "../scale.js";

/** A card with no controls drawn: the steppers' DOM is not under test here. */
const card = (now) => ({
  el: { querySelector: () => null, querySelectorAll: () => [] },
  kindNow: new Map(Object.entries(now)),
  kindScale: new Map(),
  scaleLocked: false,
});

test("a pending count is kept per kind, and dropped once it equals what runs", () => {
  const c = card({ worker: 1, mailer: 0 });

  setScale(c, "worker", 3);
  setScale(c, "mailer", 1);
  assert.deepEqual(Object.fromEntries(c.kindScale), { worker: 3, mailer: 1 }, "the scale request body");

  setScale(c, "worker", 1);
  assert.deepEqual(Object.fromEntries(c.kindScale), { mailer: 1 });
  assert.equal(valueOf(c, "worker"), 1);
});

test("a requested count is clamped to 0..MAX_SCALE", () => {
  const c = card({ worker: 1 });

  setScale(c, "worker", -4);
  assert.equal(valueOf(c, "worker"), 0);
  setScale(c, "worker", 999);
  assert.equal(valueOf(c, "worker"), 32);
});
