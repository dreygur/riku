// Process lifecycle + filesystem sandbox helpers for the dashboard E2E stress
// suite. Everything here operates on a throwaway RIKU_ROOT under os.tmpdir(),
// never the real ~/.riku, and spawns the real riku binary + real Next.js
// dashboard as detached process groups so they can be torn down with one
// signal even if a phase fails midway.

import { spawn, execFileSync, type ChildProcess } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  copyFileSync,
  chmodSync,
  rmSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import net from "node:net";

export interface SandboxHandle {
  rikuRoot: string;
  healthPort: number;
  dashboardPort: number;
  rikuApiUrl: string;
  dashboardUrl: string;
  rikuProc: ChildProcess;
  dashboardProc: ChildProcess;
  rikuLog: string[];
  dashboardLog: string[];
}

const RIKU_BINARY = join(__dirname, "..", "..", "..", "..", "target", "release", "riku");
const DASHBOARD_ROOT = join(__dirname, "..", "..", "..");
const FIXTURE_APP_DIR = join(__dirname, "..", "fixtures", "test-app");
const SHELL_PLUGIN_PATH = join(__dirname, "..", "fixtures", "shell-plugin.sh");

/** Directory skeleton matching create_directory_structure() in
 * src/cli/setup/init.rs, minus the bits that require root or interactive
 * input (systemd, acme cert generation, SSH key setup) — we don't need
 * `riku init`'s full interactive flow, just the directories the supervisor
 * and CLI commands assume exist. */
const REQUIRED_SUBDIRS = [
  "apps",
  "cache",
  "data",
  "repos",
  "envs",
  "workers",
  "workers-available",
  "workers-enabled",
  "logs",
  "nginx",
  "acme",
  "plugins",
];

/** Bind to port 0, read back the OS-assigned port, close, and return it.
 * Small TOCTOU window between this and the real bind by the spawned
 * process, acceptable for a single-worker local test run. */
export function getFreePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.unref();
    srv.on("error", reject);
    srv.listen(0, "127.0.0.1", () => {
      const address = srv.address();
      if (address === null || typeof address === "string") {
        reject(new Error("could not determine assigned port"));
        return;
      }
      const port = address.port;
      srv.close(() => resolve(port));
    });
  });
}

export function createSandboxRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "riku-e2e-"));
  for (const dir of REQUIRED_SUBDIRS) {
    mkdirSync(join(root, dir), { recursive: true });
  }
  return root;
}

/** Copy the fixture Procfile/web.sh/Dockerfile into an already-created app
 * directory (created via the real [CREATE] button, so the bare git repo
 * under repos/<app>.git also exists, matching real-world usage). Also
 * installs the `shell` runtime plugin into the sandbox's plugins/ dir and
 * sets RUNTIME=shell in the app's ENV file — the fixture app is a bare
 * shell script with no package.json/requirements.txt/etc., so none of the
 * bundled runtime plugins (node, python, ruby, go, rust-lang) would
 * `detect` it, and detection would otherwise fail with "No runtime plugin
 * matched" (see src/plugins/runtime.rs detect()). */
export function installFixtureApp(rikuRoot: string, appName: string): void {
  const appDir = join(rikuRoot, "apps", appName);
  for (const file of ["Procfile", "web.sh", "Dockerfile"]) {
    copyFileSync(join(FIXTURE_APP_DIR, file), join(appDir, file));
  }
  chmodSync(join(appDir, "web.sh"), 0o755);

  const pluginDest = join(rikuRoot, "plugins", "shell");
  copyFileSync(SHELL_PLUGIN_PATH, pluginDest);
  chmodSync(pluginDest, 0o755);

  const envDir = join(rikuRoot, "envs", appName);
  mkdirSync(envDir, { recursive: true });
  writeFileSync(join(envDir, "ENV"), "RUNTIME=shell\n");
}

function spawnTracked(
  command: string,
  args: string[],
  options: { cwd: string; env: NodeJS.ProcessEnv },
): { proc: ChildProcess; log: string[] } {
  const proc = spawn(command, args, {
    cwd: options.cwd,
    env: options.env,
    detached: true,
    stdio: ["ignore", "pipe", "pipe"],
  });

  const log: string[] = [];
  const capture = (chunk: Buffer) => {
    for (const line of chunk.toString("utf-8").split("\n")) {
      if (line.length > 0) log.push(line);
    }
  };
  proc.stdout?.on("data", capture);
  proc.stderr?.on("data", capture);

  return { proc, log };
}

async function waitForHttp(url: string, timeoutMs: number, intervalMs = 200): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown = null;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(url, { signal: AbortSignal.timeout(intervalMs) });
      if (res.status === 200) return;
      lastError = new Error(`HTTP ${res.status}`);
    } catch (e) {
      lastError = e;
    }
    await new Promise((r) => setTimeout(r, intervalMs));
  }
  throw new Error(`timed out waiting for ${url} to return 200: ${lastError}`);
}

export async function startSandbox(): Promise<SandboxHandle> {
  const rikuRoot = createSandboxRoot();
  const healthPort = await getFreePort();
  const dashboardPort = await getFreePort();
  const rikuApiUrl = `http://127.0.0.1:${healthPort}`;
  const dashboardUrl = `http://127.0.0.1:${dashboardPort}`;

  const rikuEnv: NodeJS.ProcessEnv = {
    ...process.env,
    RIKU_ROOT: rikuRoot,
    RIKU_HEALTH_PORT: String(healthPort),
    HOME: process.env.HOME ?? "/tmp",
  };

  const { proc: rikuProc, log: rikuLog } = spawnTracked(RIKU_BINARY, ["supervisor"], {
    cwd: rikuRoot,
    env: rikuEnv,
  });

  await waitForHttp(`${rikuApiUrl}/health`, 15_000);

  const dashboardEnv: NodeJS.ProcessEnv = {
    ...process.env,
    RIKU_API_URL: rikuApiUrl,
    RIKU_ROOT: rikuRoot,
    PORT: String(dashboardPort),
  };

  // `next dev` is deliberately not used here: client-side useEffect hooks
  // never fire on initial mount under this Next 16.2.9 + React 19.2.7 +
  // Turbopack dev-server combination (verified directly — a
  // window.fetch-patching init script and a literal console.log placed
  // inside the effect body both observed zero invocations for 20+ seconds,
  // while a manual page.evaluate(() => fetch(...)) against the same origin
  // succeeded instantly). `next build && next start` does not exhibit this
  // — every poll fires correctly — and matches how the dashboard actually
  // runs in production, and how tests/production_audit/dashboard already
  // tests it for the same fidelity reason.
  execFileSync("npx", ["next", "build"], { cwd: DASHBOARD_ROOT, stdio: "ignore" });

  const { proc: dashboardProc, log: dashboardLog } = spawnTracked(
    "npx",
    ["next", "start", "-p", String(dashboardPort)],
    { cwd: DASHBOARD_ROOT, env: dashboardEnv },
  );

  await waitForHttp(`${dashboardUrl}/api/health`, 60_000);

  return {
    rikuRoot,
    healthPort,
    dashboardPort,
    rikuApiUrl,
    dashboardUrl,
    rikuProc,
    dashboardProc,
    rikuLog,
    dashboardLog,
  };
}

/** SIGTERM the whole process group, escalate to SIGKILL if it doesn't exit
 * within the grace period. Each spawned process is `detached: true`, so its
 * pid is also its process group id — `-pid` signals the whole group
 * (the shell `sh -c` wrapper riku spawns for Procfile commands included). */
async function killProcessGroup(proc: ChildProcess, graceMs = 5_000): Promise<void> {
  if (proc.pid === undefined || proc.exitCode !== null) return;

  const exited = new Promise<void>((resolve) => proc.once("exit", () => resolve()));

  try {
    process.kill(-proc.pid, "SIGTERM");
  } catch {
    return; // group already gone
  }

  const timedOut = await Promise.race([
    exited.then(() => false),
    new Promise<boolean>((resolve) => setTimeout(() => resolve(true), graceMs)),
  ]);

  if (timedOut && proc.exitCode === null) {
    try {
      process.kill(-proc.pid, "SIGKILL");
    } catch {
      // already gone
    }
    await exited;
  }
}

/** Best-effort removal of any podman images this run produced. Image
 * names always follow `riku-<app>` (see container_runtime::build_and_export),
 * so we can target them precisely instead of pruning everything. */
function cleanupPodmanImage(appName: string): void {
  try {
    spawn("podman", ["rmi", "-f", `riku-${appName}`], { stdio: "ignore" });
  } catch {
    // podman not present or image never built — nothing to clean up
  }
}

export async function stopSandbox(handle: SandboxHandle, appNames: string[]): Promise<void> {
  await Promise.all([killProcessGroup(handle.rikuProc), killProcessGroup(handle.dashboardProc)]);
  for (const app of appNames) cleanupPodmanImage(app);
  rmSync(handle.rikuRoot, { recursive: true, force: true });
}

/** Count open file descriptors for a process via /proc — used to assert the
 * Hono SSE route doesn't leak a fd/watcher per abandoned connection. */
export function countOpenFds(pid: number): number {
  try {
    return readdirSync(`/proc/${pid}/fd`).length;
  } catch {
    return -1; // process gone or /proc unavailable (non-Linux) — caller should skip the assertion
  }
}

export function isProcessAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}
