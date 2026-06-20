import { watch, type FSWatcher } from "node:fs";
import { open, readdir, stat } from "node:fs/promises";
import { basename, join } from "node:path";
import { homedir } from "node:os";

export const runtime = "nodejs";

const RIKU_ROOT = process.env.RIKU_ROOT ?? join(homedir(), ".riku");

function logDir(app: string): string {
  return join(RIKU_ROOT, "logs", app);
}

// Process log files are named `{process}.{index}.log` (e.g. web.1.log,
// worker.1.log) by src/deploy/workers.rs. deploy.log lives in the same
// directory but is a separate append-only deploy-history file (see
// src/util/deploy_logger.rs) with its own `riku logs --deploy` command —
// `riku logs <app>` (the one this panel is labeled after) tails every
// *process* log except that one, so we apply the same filter here.
async function listProcessLogFiles(dir: string): Promise<string[]> {
  let entries: string[];
  try {
    entries = await readdir(dir);
  } catch {
    return [];
  }
  return entries
    .filter((name) => name.endsWith(".log") && name !== "deploy.log")
    .sort()
    .map((name) => join(dir, name));
}

function stemPrefix(path: string): string {
  return basename(path, ".log");
}

interface TrackedFile {
  offset: number;
  ino: number;
}

export async function GET(req: Request) {
  const app = new URL(req.url).searchParams.get("app");
  if (!app) {
    return new Response("missing ?app= query param", { status: 400 });
  }
  const dir = logDir(app);

  const stream = new ReadableStream({
    async start(controller) {
      let closed = false;
      const tracked = new Map<string, TrackedFile>();

      const trackNewFile = async (file: string) => {
        try {
          const st = await stat(file);
          tracked.set(file, { offset: st.size, ino: st.ino });
        } catch {
          // disappeared between listing and stat'ing; ignore
        }
      };

      // Discover the app's current process log files and seek each to EOF,
      // mirroring the CLI's open_at_end() (src/cli/apps/logs.rs): a newly
      // attached stream shows only lines written after it connects.
      for (const file of await listProcessLogFiles(dir)) {
        await trackNewFile(file);
      }

      const sendChunk = async () => {
        if (closed) return;

        // Re-list on every tick so workers added/removed by a rescale or
        // redeploy are picked up/dropped without reconnecting the stream.
        const current = await listProcessLogFiles(dir);
        for (const file of current) {
          if (!tracked.has(file)) await trackNewFile(file);
        }
        for (const file of [...tracked.keys()]) {
          if (!current.includes(file)) tracked.delete(file);
        }

        for (const file of current) {
          const state = tracked.get(file);
          if (!state) continue;
          try {
            const st = await stat(file);
            // Inode changed under us (external rotation/truncation):
            // restart from the top of the new file.
            if (st.ino !== state.ino) {
              state.ino = st.ino;
              state.offset = 0;
            }
            if (st.size <= state.offset) continue;
            const handle = await open(file, "r");
            const buf = Buffer.alloc(st.size - state.offset);
            await handle.read(buf, 0, buf.length, state.offset);
            await handle.close();
            state.offset = st.size;
            const prefix = stemPrefix(file);
            for (const line of buf.toString("utf-8").split("\n")) {
              if (line.length === 0) continue;
              controller.enqueue(
                `data: ${JSON.stringify({ line: `${prefix} | ${line}` })}\n\n`,
              );
            }
          } catch {
            // file briefly missing/rotating; ignore and retry on next tick
          }
        }
      };

      let watcher: FSWatcher | null = null;
      try {
        watcher = watch(dir, () => {
          sendChunk();
        });
      } catch {
        // logs/{app}/ doesn't exist yet — typical right after [CREATE],
        // before the first [DEPLOY] has run. fs.watch() can't watch a
        // path that doesn't exist, and never retries on its own, so
        // without the poll fallback below this connection would stay
        // open and "LIVE" forever while silently never receiving the
        // first deploy's lines, even after the directory and file get
        // created moments later.
      }

      // Poll fallback, always running alongside the watcher (not just
      // when it fails to attach): covers both the missing-directory case
      // above and the case where fs.watch fires but the directory entry
      // it reports doesn't trigger a watch on the file itself (e.g. some
      // editors/filesystems coalesce or miss rapid successive writes).
      // 250ms keeps "real-time" perception intact without re-reading the
      // file constantly.
      const poll = setInterval(sendChunk, 250);

      const heartbeat = setInterval(() => {
        if (closed) return;
        controller.enqueue(`: heartbeat\n\n`);
      }, 15_000);

      req.signal.addEventListener("abort", () => {
        closed = true;
        watcher?.close();
        clearInterval(poll);
        clearInterval(heartbeat);
        controller.close();
      });
    },
  });

  return new Response(stream, {
    headers: {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache, no-transform",
      Connection: "keep-alive",
      "X-Accel-Buffering": "no",
    },
  });
}
