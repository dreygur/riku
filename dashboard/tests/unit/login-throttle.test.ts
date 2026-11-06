import { describe, expect, it } from "vitest";
import { clientKey, lockRemaining, recordFailure, recordSuccess } from "@/lib/login-throttle";

describe("login throttle", () => {
  it("locks after five failures, doubles the lock, and clears on success", () => {
    const ip = "203.0.113.7";
    const t0 = 1_000_000;
    for (let i = 0; i < 4; i++) recordFailure(ip, t0);
    expect(lockRemaining(ip, t0), "four failures stay free").toBe(0);

    recordFailure(ip, t0);
    expect(lockRemaining(ip, t0), "fifth failure locks for 30 s").toBe(30_000);

    recordFailure(ip, t0 + 30_000);
    expect(lockRemaining(ip, t0 + 30_000), "sixth failure doubles the lock").toBe(60_000);

    recordSuccess(ip);
    expect(lockRemaining(ip, t0 + 30_000)).toBe(0);
  });

  it("keys on the last X-Forwarded-For entry, the one the router appended", () => {
    const headers = new Headers({ "x-forwarded-for": "6.6.6.6, 198.51.100.4" });
    expect(clientKey(headers)).toBe("198.51.100.4");
  });
});
