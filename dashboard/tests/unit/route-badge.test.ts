import { describe, expect, it } from "vitest";
import { routeBadge } from "@/lib/types";

describe("routeBadge", () => {
  it("names the router and whether it serves the app", () => {
    expect(routeBadge({ router: "nginx", config_exists: true, enabled: false })).toEqual({
      label: "nginx staged",
      live: false,
    });
    expect(routeBadge({ router: "caddy", site: "/home/deploy/.riku/caddy/shop.caddy" })).toEqual({
      label: "caddy live",
      live: true,
    });
    expect(routeBadge({ router: "caddy", site: null })).toEqual({ label: "caddy off", live: false });
    expect(routeBadge({ router: "unknown", error: "denied" }).live).toBe(false);
  });
});
