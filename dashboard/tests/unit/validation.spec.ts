import { test, expect } from "@playwright/test";

import { validateAppName } from "../../server/validation";

// Pure unit tests for the path-traversal guard `validateAppName`.
// No browser, no server — Playwright runs these as plain TypeScript.

test.describe("validateAppName — accepts legal app names", () => {
  const valid = ["myapp", "my-app", "my_app", "app.1", "App2"];

  for (const name of valid) {
    test(`returns ${JSON.stringify(name)} unchanged`, () => {
      expect(validateAppName(name)).toBe(name);
    });
  }
});

test.describe("validateAppName — rejects illegal / traversal names", () => {
  // Each entry is [label, input]. null/undefined need explicit cases.
  test("rejects empty string", () => {
    expect(validateAppName("")).toBeNull();
  });

  test("rejects null", () => {
    expect(validateAppName(null)).toBeNull();
  });

  test("rejects undefined", () => {
    expect(validateAppName(undefined)).toBeNull();
  });

  const rejected = [
    "../etc", // parent traversal
    "a/b", // path separator
    "a/../b", // embedded traversal
    "..", // pure parent dir
    ".", // current dir
    "...", // dots only
    "app name", // space
    "app$", // shell metachar
    "app;rm", // command injection attempt
    "a\\b", // backslash separator
    "/app", // leading slash (absolute path)
  ];

  for (const name of rejected) {
    test(`rejects ${JSON.stringify(name)}`, () => {
      expect(validateAppName(name)).toBeNull();
    });
  }
});
