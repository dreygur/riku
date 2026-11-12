import { test } from "node:test";
import assert from "node:assert/strict";

import { fmtClock, parseLogEvent } from "../format.js";

test("a log event carries the line's own time, shown as a local clock", () => {
  const ev = parseLogEvent("web.1\t2026-11-12T08:14:03Z\tlistening\ton :37139");

  assert.equal(ev.tag, "web.1");
  assert.equal(ev.text, "listening\ton :37139", "tabs in the text survive");
  assert.equal(ev.ts.toISOString(), "2026-11-12T08:14:03.000Z");
  const local = new Date(Date.UTC(2026, 10, 12, 8, 14, 3));
  const pad = (n) => String(n).padStart(2, "0");
  assert.equal(fmtClock(ev.ts), `${pad(local.getHours())}:${pad(local.getMinutes())}:${pad(local.getSeconds())}`);
});

test("a line with no stamp has no time, not the time it arrived", () => {
  const ev = parseLogEvent("deploy\t\t[WARN] cache missing");

  assert.equal(ev.ts, null);
  assert.equal(ev.text, "[WARN] cache missing");
});
