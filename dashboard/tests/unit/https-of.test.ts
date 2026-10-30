import { describe, expect, it } from "vitest";
import { httpsOf, type AppState } from "@/lib/types";

const app = (value?: string) => ({ routing: value === undefined ? {} : { RIKU_HTTPS_ONLY: value } }) as AppState;

describe("httpsOf", () => {
  it("matches the server's on/off rule", () => {
    expect(["1", "true", " TRUE ", "yes", "on"].map((v) => httpsOf(app(v)))).toEqual([true, true, true, true, true]);
    expect(["", "0", "false", "no", "enabled"].map((v) => httpsOf(app(v)))).toEqual([false, false, false, false, false]);
    expect(httpsOf(app())).toBe(false);
  });
});
