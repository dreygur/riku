import { defineConfig } from "@playwright/test";

// This suite manages its own servers (real riku supervisor + real Next.js
// dashboard, see tests/e2e-dashboard/helpers/sandbox.ts) — no Playwright
// `webServer` entry, since each test run needs a fresh sandboxed RIKU_ROOT
// and dynamically allocated ports, not one fixed server for the whole run.
export default defineConfig({
  testDir: "./tests/e2e-dashboard",
  timeout: 5 * 60_000, // real podman builds and supervisor startup are slow
  expect: { timeout: 10_000 },
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [["list"]],
  use: {
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
});
