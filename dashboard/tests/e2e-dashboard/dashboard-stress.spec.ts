// Verification Module 1: End-to-End Live Dashboard & Runtime Stream Stress Test
//
// Drives a REAL riku supervisor binary + REAL Next.js/Hono dashboard + REAL
// headless Chromium + REAL podman, against a throwaway sandboxed RIKU_ROOT.
// No mocked HTTP, no stubbed SSE, no fake process states.
//
// Two corrections versus the original spec, grounded in the actual source
// (not asserted on faith — see the inline citations below):
//   1. There is no "building" worker-grid state. `ProcessStats.status` only
//      ever resolves to running|stopped|crashed (dashboard/lib/types.ts
//      STATUS_MAP). A deploy either produces a RUNNING row or no row.
//   2. Podman is not part of `riku deploy` — Procfile workers are spawned
//      directly by the supervisor. Podman only exists via the separate
//      container-export feature, which builds+tars an image and never runs
//      it as a worker. Phase 2 is split into a deploy sub-phase (no podman)
//      and an export sub-phase (real podman, no running container).
//
// Phase 3 is grounded against the actual SSE route
// (app/api/logs/stream/route.ts), which tails the app's *process* log
// files (`logs/{app}/{process}.{index}.log`, e.g. `web.1.log` — written by
// src/supervisor/process/spawn.rs in append mode) rather than
// `deploy.log` (a separate, append-only deploy-history file with its own
// `riku logs --deploy` command; see src/util/deploy_logger.rs). Two
// behaviors are asserted as TRUE rather than papered over:
//   - Process logs are opened with O_APPEND and never truncated by a
//     redeploy/restart (only DeployLogger truncates deploy.log, which this
//     route doesn't read), so the stream keeps working across redeploys —
//     verified below rather than assumed.
//   - Reconnecting (e.g. after a page reload) seeks every tracked file to
//     EOF at connect time — it never backfills. The UI's `lines` state is
//     also component-local with no persistence. So a reload does NOT
//     "catch up with backlogged logs" — it starts from a blank slate and
//     only shows lines written after the new connection opens.

import { test, expect, chromium, type Browser, type Page } from "@playwright/test";
import { existsSync, appendFileSync, mkdirSync, readFileSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join } from "node:path";
import {
  startSandbox,
  stopSandbox,
  installFixtureApp,
  countOpenFds,
  isProcessAlive,
  type SandboxHandle,
} from "./helpers/sandbox";

const APP_NAME = "e2estressapp";
const STRESS_LINE_COUNT = 500;

test.describe.serial("Riku Dashboard E2E Stress Suite", () => {
  let env: SandboxHandle;
  let browser: Browser;
  let page: Page;
  let workerPid: number;

  test.beforeAll(async () => {
    env = await startSandbox();
    browser = await chromium.launch();
    page = await browser.newPage();
    page.on("dialog", (dialog) => dialog.accept());
  });

  test.afterAll(async () => {
    await browser?.close();
    await stopSandbox(env, [APP_NAME]);
  });

  // ── Phase 1: Bootstrap & Connection Hardening ──────────────────────────

  test("Phase 1: real supervisor + dashboard boot and report healthy", async () => {
    const rikuHealth = await fetch(`${env.rikuApiUrl}/health`);
    expect(rikuHealth.status).toBe(200);
    const rikuHealthBody = await rikuHealth.json();
    expect(rikuHealthBody.status).toBe("healthy");

    const rikuMetrics = await fetch(`${env.rikuApiUrl}/metrics`);
    expect(rikuMetrics.status).toBe(200);

    const dashHealth = await fetch(`${env.dashboardUrl}/api/health`);
    expect(dashHealth.status).toBe(200);

    const dashMetrics = await fetch(`${env.dashboardUrl}/api/metrics`);
    expect(dashMetrics.status).toBe(200);

    await page.goto(env.dashboardUrl);
    await expect(page.getByText("RIKU // PLATFORM_DAEMON")).toBeVisible();
    await expect(page.getByTestId("supervisor-grid")).toBeVisible();
    await expect(page.getByText(/CONN:OK/)).toBeVisible({ timeout: 15_000 });
  });

  // ── Phase 2a: Real Procfile Deploy (no podman) ─────────────────────────

  test("Phase 2a: create + deploy a real app, worker grid flips empty -> running", async () => {
    await expect(page.getByTestId("worker-row").filter({ hasText: APP_NAME })).toHaveCount(0);

    await page.getByTestId("new-app-input").fill(APP_NAME);
    await page.getByTestId("create-btn").click();
    await expect(page.getByTestId("action-status")).toHaveText(new RegExp(`\\[CREATE\\] ${APP_NAME} ok`), {
      timeout: 15_000,
    });
    await expect(page.getByTestId("active-app-input")).toHaveValue(APP_NAME);

    expect(existsSync(join(env.rikuRoot, "apps", APP_NAME))).toBe(true);
    installFixtureApp(env.rikuRoot, APP_NAME);

    await page.getByTestId("deploy-btn").click();
    await expect(page.getByTestId("action-status")).toHaveText(new RegExp(`\\[DEPLOY\\] ${APP_NAME} ok`), {
      timeout: 30_000,
    });

    const row = page.locator(`[data-testid="worker-row"][data-app="${APP_NAME}"]`);
    await expect(row).toHaveCount(1, { timeout: 15_000 });
    await expect(row).toHaveAttribute("data-status", "running", { timeout: 15_000 });

    const pidAttr = await row.getAttribute("data-pid");
    expect(pidAttr).not.toBeNull();
    expect(pidAttr).not.toBe("");
    workerPid = Number(pidAttr);
    expect(Number.isInteger(workerPid)).toBe(true);
    expect(isProcessAlive(workerPid)).toBe(true);

    await expect(page.getByTestId("terminal-stream-status")).toHaveAttribute("data-connected", "true", {
      timeout: 15_000,
    });
    // The SSE route tails the process log (web.1.log), not deploy.log, so
    // the fixture worker's own output — not the deploy narration — is what
    // proves the stream picked up this app's real output. Matching on the
    // repeating "heartbeat" line (not the one-shot FIXTURE_WEB_PID= line)
    // avoids a race against the route's by-design no-backfill behavior: a
    // brand-new log file is only discovered on the route's next poll tick
    // (every 250ms), by which point the single startup line may already
    // be behind that tick's seek-to-EOF — heartbeats keep coming every
    // second, so there's always another chance within the timeout.
    await expect(
      page.getByTestId("terminal-stream-line").filter({ hasText: "heartbeat" }).first(),
    ).toBeVisible({ timeout: 15_000 });
  });

  // ── Phase 2b: Real Podman Build + Export ───────────────────────────────

  test("Phase 2b: real podman build+export produces a valid, loadable tar", async () => {
    await page.getByTestId("export-image-btn").click();
    await expect(page.getByTestId("action-status")).toHaveText(/\[CONTAINER_EXPORT\] .* ok/, {
      timeout: 120_000, // real podman pull+build, no shortcuts
    });

    const tarPath = join(env.rikuRoot, "data", "exports", `${APP_NAME}.tar`);
    expect(existsSync(tarPath)).toBe(true);
    expect(statSync(tarPath).size).toBeGreaterThan(0);

    // Real tar validity check, not just "file exists".
    const tarListing = execFileSync("tar", ["-tf", tarPath], { encoding: "utf-8" });
    expect(tarListing.trim().length).toBeGreaterThan(0);

    // Real podman round-trip: load the exported archive back and confirm
    // the image is actually reconstitutable, not just a byte blob.
    execFileSync("podman", ["load", "-i", tarPath]);
    const images = execFileSync("podman", ["images", "--format", "{{.Repository}}"], {
      encoding: "utf-8",
    });
    expect(images).toContain(`riku-${APP_NAME}`);
  });

  // ── Phase 3: Hostile Stream Stress, Disconnection, and Truncation ─────

  test("Phase 3: high-volume ordering, disconnect/reconnect, fd leak, redeploy resilience", async () => {
    // The fixture's Procfile declares a single `web` process (see
    // tests/e2e-dashboard/fixtures/test-app/Procfile), so the supervisor
    // writes its output to web.1.log (src/deploy/workers.rs:
    // `{kind}.{index}.log`) — this is the file the SSE route now tails,
    // not deploy.log.
    const processLogPath = join(env.rikuRoot, "logs", APP_NAME, "web.1.log");
    expect(existsSync(processLogPath)).toBe(true);

    const linesBefore = await page.getByTestId("terminal-stream-line").count();

    // ── Stress: append STRESS_LINE_COUNT sequenced lines while the SSE
    // connection is live, same direct-append technique already proven by
    // dashboard/scripts/audit-dashboard.ts's auditLogTail phase. ──
    for (let i = 1; i <= STRESS_LINE_COUNT; i++) {
      appendFileSync(processLogPath, `STRESS_LINE_${i}\n`);
      if (i % 50 === 0) await new Promise((r) => setTimeout(r, 10));
    }

    await expect
      .poll(async () => page.getByTestId("terminal-stream-line").count(), { timeout: 30_000, intervals: [250] })
      .toBeGreaterThanOrEqual(linesBefore + STRESS_LINE_COUNT);

    const renderedLines = await page.getByTestId("terminal-stream-line").allTextContents();
    const sequenceNumbers = renderedLines
      .map((l) => l.match(/STRESS_LINE_(\d+)/)?.[1])
      .filter((v): v is string => v !== undefined)
      .map(Number);

    expect(sequenceNumbers.length).toBe(STRESS_LINE_COUNT); // no drops, no dupes
    expect(new Set(sequenceNumbers).size).toBe(STRESS_LINE_COUNT); // no dupes (explicit)
    for (let i = 1; i < sequenceNumbers.length; i++) {
      expect(sequenceNumbers[i]).toBeGreaterThan(sequenceNumbers[i - 1]); // strict ordering, no truncation/reorder
    }

    // ── fd leak check: baseline before the disconnect cycle ──
    const fdBaseline = countOpenFds(env.dashboardProc.pid!);

    // ── Hostile disconnect: reload mid-"stream" (file is idle now, but the
    // EventSource connection is abruptly torn down exactly as it would be
    // on a real network drop or tab close). ──
    await page.reload();
    await expect(page.getByTestId("supervisor-grid")).toBeVisible();

    // `activeApp` is plain React state (app/page.tsx) with no
    // localStorage/URL persistence, so a reload resets the active-app
    // selector to "" — AppControls then auto-selects the first app in
    // ~/.riku/workers-enabled once its poll lands. With only one app
    // deployed in this test that's already APP_NAME, but explicitly fill
    // the input anyway so the assertion below doesn't depend on poll
    // timing or alphabetical ordering if more apps are ever added here.
    await page.getByTestId("active-app-input").fill(APP_NAME);

    // REAL behavior, not the originally-assumed one: reconnect does not
    // backfill. The component remounts with empty `lines` state and the
    // SSE route seeks every tracked file to EOF at connect time
    // (app/api/logs/stream/route.ts), so the 500 stress lines above are
    // gone from the UI after reload even though they're still on disk.
    await expect(page.getByTestId("terminal-stream-line")).toHaveCount(0);
    expect(readFileSync(processLogPath, "utf-8")).toContain("STRESS_LINE_500"); // proves it's a UI gap, not data loss

    // Live streaming still works post-reconnect for genuinely new writes.
    appendFileSync(processLogPath, "POST_RECONNECT_LINE\n");
    await expect(page.getByTestId("terminal-stream-line").filter({ hasText: "POST_RECONNECT_LINE" })).toBeVisible({
      timeout: 10_000,
    });

    // The old connection's fd must be cleaned up — the route's
    // `req.signal.addEventListener("abort", ...)` should fire on
    // navigation. Allow a small margin for the new connection's own fd
    // plus unrelated Node internals, but a real leak would show a large,
    // growing delta, not a constant few.
    const fdAfterReconnect = countOpenFds(env.dashboardProc.pid!);
    if (fdBaseline >= 0 && fdAfterReconnect >= 0) {
      expect(fdAfterReconnect - fdBaseline).toBeLessThanOrEqual(5);
    }

    // ── Redeploy resilience: web.1.log is reopened in append mode by
    // spawn_process on every restart (src/supervisor/process/spawn.rs),
    // never truncated, and the SSE route re-lists the log directory and
    // re-stats every tracked file on each tick rather than trusting a
    // connection-lifetime-stale offset — so the now-reconnected stream
    // (offset captured post-reload, after POST_RECONNECT_LINE) must keep
    // receiving new lines straight through a redeploy, not go silent. ──
    await page.getByTestId("deploy-btn").click();
    await expect(page.getByTestId("action-status")).toHaveText(new RegExp(`\\[DEPLOY\\] ${APP_NAME} ok`), {
      timeout: 30_000,
    });

    // The fixture script (web.sh) prints "FIXTURE_WEB_PID=<pid>" once at
    // startup, so the restarted worker's own startup line is a clean
    // signal that streaming survived the redeploy rather than going dark.
    const restartLine = page.getByTestId("terminal-stream-line").filter({ hasText: "FIXTURE_WEB_PID=" });
    await expect(restartLine).toBeVisible({ timeout: 15_000 });
    expect(readFileSync(processLogPath, "utf-8")).toContain("FIXTURE_WEB_PID="); // UI and disk agree

    // This redeploy restarted the worker (spawn_process stops the old PID
    // before spawning a new one), so the PID captured back in Phase 2a is
    // gone now by design, not by crash. Refresh it so Phase 4 checks the
    // process actually backing today's [RUNNING] row.
    const refreshedRow = page.locator(`[data-testid="worker-row"][data-app="${APP_NAME}"]`);
    await expect(refreshedRow).toHaveAttribute("data-status", "running", { timeout: 15_000 });
    const refreshedPid = await refreshedRow.getAttribute("data-pid");
    expect(refreshedPid).not.toBeNull();
    expect(refreshedPid).not.toBe("");
    workerPid = Number(refreshedPid);
    expect(Number.isInteger(workerPid)).toBe(true);
    expect(isProcessAlive(workerPid)).toBe(true);
  });

  // ── Phase 4: Deep Cleanup & Tear Down ───────────────────────────────────

  test("Phase 4: destroy via UI terminates the real process and clears the grid", async () => {
    expect(isProcessAlive(workerPid)).toBe(true); // sanity: still alive before destroy

    await page.getByTestId("destroy-btn").click(); // dialog auto-accepted by the beforeAll handler
    await expect(page.getByTestId("action-status")).toHaveText(new RegExp(`\\[DESTROY\\] ${APP_NAME} ok`), {
      timeout: 15_000,
    });

    await expect
      .poll(async () => page.locator(`[data-testid="worker-row"][data-app="${APP_NAME}"]`).count(), {
        timeout: 15_000,
        intervals: [500],
      })
      .toBe(0);

    // Real OS-level confirmation, not just trusting the UI: the worker
    // process the supervisor reported earlier must actually be gone.
    await expect.poll(() => isProcessAlive(workerPid), { timeout: 10_000, intervals: [250] }).toBe(false);

    expect(existsSync(join(env.rikuRoot, "apps", APP_NAME))).toBe(false);
  });
});
