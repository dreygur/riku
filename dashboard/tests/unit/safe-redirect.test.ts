import { describe, expect, it } from "vitest";
import { safeRedirectPath } from "@/lib/safe-redirect";

describe("safeRedirectPath", () => {
  it("keeps a same-origin path with its query", () => {
    expect(safeRedirectPath("/apps/shop?tab=logs")).toBe("/apps/shop?tab=logs");
  });

  it("refuses targets that leave the dashboard origin", () => {
    const hostile = ["https://evil.example", "//evil.example", "/\\evil.example", "\\/evil.example", "javascript:alert(1)"];
    expect(hostile.map(safeRedirectPath)).toEqual(hostile.map(() => "/"));
    expect(safeRedirectPath(null)).toBe("/");
  });
});
