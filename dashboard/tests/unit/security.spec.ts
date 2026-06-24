import { test, expect } from "@playwright/test";

import { allowedOrigin, dashboardToken } from "../../server/security";

// Pure unit tests for the exported env-reading helpers in server/security.ts.
// Each test saves and restores the env var it touches so cases stay isolated.

const DEFAULT_ORIGIN = "http://127.0.0.1:3000";

function withEnv(key: string, value: string | undefined, fn: () => void): void {
  const original = process.env[key];
  try {
    if (value === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = value;
    }
    fn();
  } finally {
    if (original === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = original;
    }
  }
}

test.describe("allowedOrigin", () => {
  test("returns default when RIKU_DASHBOARD_ORIGIN is unset", () => {
    withEnv("RIKU_DASHBOARD_ORIGIN", undefined, () => {
      expect(allowedOrigin()).toBe(DEFAULT_ORIGIN);
    });
  });

  test("returns default when RIKU_DASHBOARD_ORIGIN is empty", () => {
    withEnv("RIKU_DASHBOARD_ORIGIN", "", () => {
      expect(allowedOrigin()).toBe(DEFAULT_ORIGIN);
    });
  });

  test("returns default when RIKU_DASHBOARD_ORIGIN is whitespace only", () => {
    withEnv("RIKU_DASHBOARD_ORIGIN", "   ", () => {
      expect(allowedOrigin()).toBe(DEFAULT_ORIGIN);
    });
  });

  test("returns configured value when RIKU_DASHBOARD_ORIGIN is set", () => {
    withEnv("RIKU_DASHBOARD_ORIGIN", "https://riku.example.com", () => {
      expect(allowedOrigin()).toBe("https://riku.example.com");
    });
  });
});

test.describe("dashboardToken", () => {
  test("returns null when RIKU_DASHBOARD_TOKEN is unset", () => {
    withEnv("RIKU_DASHBOARD_TOKEN", undefined, () => {
      expect(dashboardToken()).toBeNull();
    });
  });

  test("returns null when RIKU_DASHBOARD_TOKEN is empty", () => {
    withEnv("RIKU_DASHBOARD_TOKEN", "", () => {
      expect(dashboardToken()).toBeNull();
    });
  });

  test("returns null when RIKU_DASHBOARD_TOKEN is whitespace only", () => {
    withEnv("RIKU_DASHBOARD_TOKEN", "   ", () => {
      expect(dashboardToken()).toBeNull();
    });
  });

  test("returns trimmed token when RIKU_DASHBOARD_TOKEN is set", () => {
    withEnv("RIKU_DASHBOARD_TOKEN", "  s3cr3t-token  ", () => {
      expect(dashboardToken()).toBe("s3cr3t-token");
    });
  });
});
