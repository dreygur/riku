# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: dashboard-stress.spec.ts >> Riku Dashboard E2E Stress Suite >> Phase 1: real supervisor + dashboard boot and report healthy
- Location: tests/e2e-dashboard/dashboard-stress.spec.ts:70:7

# Error details

```
Error: expect(locator).toBeVisible() failed

Locator: getByText(/CONN:OK/)
Expected: visible
Timeout: 15000ms
Error: element(s) not found

Call log:
  - Expect "toBeVisible" with timeout 15000ms
  - waiting for getByText(/CONN:OK/)

```

```yaml
- banner:
  - heading "RIKU // PLATFORM_DAEMON" [level=1]
  - text: "[CONN:OFFLINE] UP:--"
- text: APP_CTL //
- textbox: myapp
- button "[DEPLOY]"
- button "[RESTART]"
- button "[STOP]"
- button "[DESTROY]"
- button "[EXPORT IMAGE]"
- text: "|"
- textbox "new app name"
- button "[CREATE]" [disabled]
- text: SUPERVISOR_METRICS loading...
- table:
  - rowgroup:
    - row "APP PROC PID RSS MEMORY CPU % RESTARTS STATUS":
      - columnheader "APP"
      - columnheader "PROC"
      - columnheader "PID"
      - columnheader "RSS MEMORY"
      - columnheader "CPU %"
      - columnheader "RESTARTS"
      - columnheader "STATUS"
  - rowgroup:
    - row "no workers supervised":
      - cell "no workers supervised"
- text: CLIENT_PLUGINS // 0 installed none installed (~/.riku/client-plugins/) HOOK_PLUGINS // 0 installed
- button "[INSTALL RUNTIMES]"
- text: none installed (~/.riku/plugins/) NETWORK_TLS // loading...
- table:
  - rowgroup:
    - row "APP SERVER_NAME UPSTREAM TLS":
      - columnheader "APP"
      - columnheader "SERVER_NAME"
      - columnheader "UPSTREAM"
      - columnheader "TLS"
  - rowgroup:
    - row "no nginx vhosts configured":
      - cell "no nginx vhosts configured"
- text: LOG_STREAM_BUFFER [OFFLINE] 0 lines
- button "[SCROLL:ON]"
- text: connecting to log stream... ENV_CFG // myapp
- table:
  - rowgroup:
    - row "KEY VALUE":
      - columnheader "KEY"
      - columnheader "VALUE"
      - columnheader
  - rowgroup:
    - row "loading...":
      - cell "loading..."
    - row "[ADD]":
      - cell:
        - textbox "KEY"
      - cell:
        - textbox "VALUE"
      - cell "[ADD]":
        - button "[ADD]" [disabled]
```

# Test source

```ts
  1   | // Verification Module 1: End-to-End Live Dashboard & Runtime Stream Stress Test
  2   | //
  3   | // Drives a REAL riku supervisor binary + REAL Next.js/Hono dashboard + REAL
  4   | // headless Chromium + REAL podman, against a throwaway sandboxed RIKU_ROOT.
  5   | // No mocked HTTP, no stubbed SSE, no fake process states.
  6   | //
  7   | // Two corrections versus the original spec, grounded in the actual source
  8   | // (not asserted on faith — see the inline citations below):
  9   | //   1. There is no "building" worker-grid state. `ProcessStats.status` only
  10  | //      ever resolves to running|stopped|crashed (dashboard/lib/types.ts
  11  | //      STATUS_MAP). A deploy either produces a RUNNING row or no row.
  12  | //   2. Podman is not part of `riku deploy` — Procfile workers are spawned
  13  | //      directly by the supervisor. Podman only exists via the separate
  14  | //      container-export feature, which builds+tars an image and never runs
  15  | //      it as a worker. Phase 2 is split into a deploy sub-phase (no podman)
  16  | //      and an export sub-phase (real podman, no running container).
  17  | //
  18  | // Two real defects were discovered while grounding Phase 3 against the
  19  | // actual SSE route (app/api/logs/stream/route.ts) and DeployLogger
  20  | // (src/util/deploy_logger.rs), and are asserted as TRUE behavior rather
  21  | // than papered over:
  22  | //   - DeployLogger::new() truncates deploy.log on every new deploy. The SSE
  23  | //     route tracks a monotonically-increasing `offset` per connection with
  24  | //     `if (size <= offset) return`. After a truncation, the new (smaller)
  25  | //     file size is almost always <= the stale offset from before the
  26  | //     truncation, so the stream goes permanently silent for that
  27  | //     connection even though the file is still being written to.
  28  | //   - Reconnecting (e.g. after a page reload) computes `offset = current
  29  | //     file size` at connect time — it never backfills. The UI's `lines`
  30  | //     state is also component-local with no persistence. So a reload does
  31  | //     NOT "catch up with backlogged logs" — it starts from a blank slate
  32  | //     and only shows lines written after the new connection opens.
  33  | 
  34  | import { test, expect, chromium, type Browser, type Page } from "@playwright/test";
  35  | import { existsSync, appendFileSync, mkdirSync, readFileSync, statSync } from "node:fs";
  36  | import { execFileSync } from "node:child_process";
  37  | import { join } from "node:path";
  38  | import {
  39  |   startSandbox,
  40  |   stopSandbox,
  41  |   installFixtureApp,
  42  |   countOpenFds,
  43  |   isProcessAlive,
  44  |   type SandboxHandle,
  45  | } from "./helpers/sandbox";
  46  | 
  47  | const APP_NAME = "e2estressapp";
  48  | const STRESS_LINE_COUNT = 500;
  49  | 
  50  | test.describe.serial("Riku Dashboard E2E Stress Suite", () => {
  51  |   let env: SandboxHandle;
  52  |   let browser: Browser;
  53  |   let page: Page;
  54  |   let workerPid: number;
  55  | 
  56  |   test.beforeAll(async () => {
  57  |     env = await startSandbox();
  58  |     browser = await chromium.launch();
  59  |     page = await browser.newPage();
  60  |     page.on("dialog", (dialog) => dialog.accept());
  61  |   });
  62  | 
  63  |   test.afterAll(async () => {
  64  |     await browser?.close();
  65  |     await stopSandbox(env, [APP_NAME]);
  66  |   });
  67  | 
  68  |   // ── Phase 1: Bootstrap & Connection Hardening ──────────────────────────
  69  | 
  70  |   test("Phase 1: real supervisor + dashboard boot and report healthy", async () => {
  71  |     const rikuHealth = await fetch(`${env.rikuApiUrl}/health`);
  72  |     expect(rikuHealth.status).toBe(200);
  73  |     const rikuHealthBody = await rikuHealth.json();
  74  |     expect(rikuHealthBody.status).toBe("healthy");
  75  | 
  76  |     const rikuMetrics = await fetch(`${env.rikuApiUrl}/metrics`);
  77  |     expect(rikuMetrics.status).toBe(200);
  78  | 
  79  |     const dashHealth = await fetch(`${env.dashboardUrl}/api/health`);
  80  |     expect(dashHealth.status).toBe(200);
  81  | 
  82  |     const dashMetrics = await fetch(`${env.dashboardUrl}/api/metrics`);
  83  |     expect(dashMetrics.status).toBe(200);
  84  | 
  85  |     await page.goto(env.dashboardUrl);
  86  |     await expect(page.getByText("RIKU // PLATFORM_DAEMON")).toBeVisible();
  87  |     await expect(page.getByTestId("supervisor-grid")).toBeVisible();
> 88  |     await expect(page.getByText(/CONN:OK/)).toBeVisible({ timeout: 15_000 });
      |                                             ^ Error: expect(locator).toBeVisible() failed
  89  |   });
  90  | 
  91  |   // ── Phase 2a: Real Procfile Deploy (no podman) ─────────────────────────
  92  | 
  93  |   test("Phase 2a: create + deploy a real app, worker grid flips empty -> running", async () => {
  94  |     await expect(page.getByTestId("worker-row").filter({ hasText: APP_NAME })).toHaveCount(0);
  95  | 
  96  |     await page.getByTestId("new-app-input").fill(APP_NAME);
  97  |     await page.getByTestId("create-btn").click();
  98  |     await expect(page.getByTestId("action-status")).toHaveText(new RegExp(`\\[CREATE\\] ${APP_NAME} ok`), {
  99  |       timeout: 15_000,
  100 |     });
  101 |     await expect(page.getByTestId("active-app-input")).toHaveValue(APP_NAME);
  102 | 
  103 |     expect(existsSync(join(env.rikuRoot, "apps", APP_NAME))).toBe(true);
  104 |     installFixtureApp(env.rikuRoot, APP_NAME);
  105 | 
  106 |     await page.getByTestId("deploy-btn").click();
  107 |     await expect(page.getByTestId("action-status")).toHaveText(new RegExp(`\\[DEPLOY\\] ${APP_NAME} ok`), {
  108 |       timeout: 30_000,
  109 |     });
  110 | 
  111 |     const row = page.getByTestId("worker-row").filter({ has: page.locator(`[data-app="${APP_NAME}"]`) });
  112 |     await expect(row).toHaveCount(1, { timeout: 15_000 });
  113 |     await expect(row).toHaveAttribute("data-status", "running", { timeout: 15_000 });
  114 | 
  115 |     const pidAttr = await row.getAttribute("data-pid");
  116 |     expect(pidAttr).not.toBeNull();
  117 |     expect(pidAttr).not.toBe("");
  118 |     workerPid = Number(pidAttr);
  119 |     expect(Number.isInteger(workerPid)).toBe(true);
  120 |     expect(isProcessAlive(workerPid)).toBe(true);
  121 | 
  122 |     await expect(page.getByTestId("terminal-stream-status")).toHaveAttribute("data-connected", "true", {
  123 |       timeout: 15_000,
  124 |     });
  125 |     await expect(
  126 |       page.getByTestId("terminal-stream-line").filter({ hasText: `Deploying app '${APP_NAME}'` }),
  127 |     ).toBeVisible({ timeout: 15_000 });
  128 |   });
  129 | 
  130 |   // ── Phase 2b: Real Podman Build + Export ───────────────────────────────
  131 | 
  132 |   test("Phase 2b: real podman build+export produces a valid, loadable tar", async () => {
  133 |     await page.getByTestId("export-image-btn").click();
  134 |     await expect(page.getByTestId("action-status")).toHaveText(/\[CONTAINER_EXPORT\] .* ok/, {
  135 |       timeout: 120_000, // real podman pull+build, no shortcuts
  136 |     });
  137 | 
  138 |     const tarPath = join(env.rikuRoot, "data", "exports", `${APP_NAME}.tar`);
  139 |     expect(existsSync(tarPath)).toBe(true);
  140 |     expect(statSync(tarPath).size).toBeGreaterThan(0);
  141 | 
  142 |     // Real tar validity check, not just "file exists".
  143 |     const tarListing = execFileSync("tar", ["-tf", tarPath], { encoding: "utf-8" });
  144 |     expect(tarListing.trim().length).toBeGreaterThan(0);
  145 | 
  146 |     // Real podman round-trip: load the exported archive back and confirm
  147 |     // the image is actually reconstitutable, not just a byte blob.
  148 |     execFileSync("podman", ["load", "-i", tarPath]);
  149 |     const images = execFileSync("podman", ["images", "--format", "{{.Repository}}"], {
  150 |       encoding: "utf-8",
  151 |     });
  152 |     expect(images).toContain(`riku-${APP_NAME}`);
  153 |   });
  154 | 
  155 |   // ── Phase 3: Hostile Stream Stress, Disconnection, and Truncation ─────
  156 | 
  157 |   test("Phase 3: high-volume ordering, disconnect/reconnect, fd leak, truncation defect", async () => {
  158 |     const deployLogPath = join(env.rikuRoot, "logs", APP_NAME, "deploy.log");
  159 |     expect(existsSync(deployLogPath)).toBe(true);
  160 | 
  161 |     const linesBefore = await page.getByTestId("terminal-stream-line").count();
  162 | 
  163 |     // ── Stress: append STRESS_LINE_COUNT sequenced lines while the SSE
  164 |     // connection is live, same direct-append technique already proven by
  165 |     // dashboard/scripts/audit-dashboard.ts's auditLogTail phase. ──
  166 |     for (let i = 1; i <= STRESS_LINE_COUNT; i++) {
  167 |       appendFileSync(deployLogPath, `STRESS_LINE_${i}\n`);
  168 |       if (i % 50 === 0) await new Promise((r) => setTimeout(r, 10));
  169 |     }
  170 | 
  171 |     await expect
  172 |       .poll(async () => page.getByTestId("terminal-stream-line").count(), { timeout: 30_000, intervals: [250] })
  173 |       .toBeGreaterThanOrEqual(linesBefore + STRESS_LINE_COUNT);
  174 | 
  175 |     const renderedLines = await page.getByTestId("terminal-stream-line").allTextContents();
  176 |     const sequenceNumbers = renderedLines
  177 |       .map((l) => l.match(/STRESS_LINE_(\d+)/)?.[1])
  178 |       .filter((v): v is string => v !== undefined)
  179 |       .map(Number);
  180 | 
  181 |     expect(sequenceNumbers.length).toBe(STRESS_LINE_COUNT); // no drops, no dupes
  182 |     expect(new Set(sequenceNumbers).size).toBe(STRESS_LINE_COUNT); // no dupes (explicit)
  183 |     for (let i = 1; i < sequenceNumbers.length; i++) {
  184 |       expect(sequenceNumbers[i]).toBeGreaterThan(sequenceNumbers[i - 1]); // strict ordering, no truncation/reorder
  185 |     }
  186 | 
  187 |     // ── fd leak check: baseline before the disconnect cycle ──
  188 |     const fdBaseline = countOpenFds(env.dashboardProc.pid!);
```