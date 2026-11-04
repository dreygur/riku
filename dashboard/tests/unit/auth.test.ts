import { describe, expect, it } from "vitest";
import { createSessionToken, revokeAllSessions, verifySessionToken } from "@/lib/auth";

describe("session tokens", () => {
  it("rejects every token issued before a logout", async () => {
    const secret = "salt:hash";
    const before = await createSessionToken(secret);
    expect(await verifySessionToken(before, secret), "fresh token verifies").toBe(true);

    revokeAllSessions();

    expect(await verifySessionToken(before, secret), "revoked token still verifies").toBe(false);
    const after = await createSessionToken(secret);
    expect(await verifySessionToken(after, secret), "token issued after logout").toBe(true);
  });
});
