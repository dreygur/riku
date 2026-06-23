import { watch, type FSWatcher } from "node:fs";
import { open, stat } from "node:fs/promises";
import { join } from "node:path";
import { homedir } from "node:os";
import { validateAppName } from "@/server/validation";

export const runtime = "nodejs";

const RIKU_ROOT = process.env.RIKU_ROOT ?? join(homedir(), ".riku");

// riku has no SSE/streaming hook for deploy logs (src/util/deploy_logger.rs
// only appends to a flat file). We tail logs/{app}/deploy.log ourselves.
function deployLogPath(app: string): string {
  return join(RIKU_ROOT, "logs", app, "deploy.log");
}

export async function GET(req: Request) {
  const app = new URL(req.url).searchParams.get("app");
  if (!app) {
    return new Response("missing ?app= query param", { status: 400 });
  }
  const safeApp = validateAppName(app);
  if (!safeApp) {
    return new Response("invalid app name", { status: 400 });
  }
  const file = deployLogPath(safeApp);

  const stream = new ReadableStream({
    async start(controller) {
      let offset = 0;
      try {
        offset = (await stat(file)).size;
      } catch {
        // file doesn't exist yet; start at 0 and pick up writes once created
      }

      let watcher: FSWatcher | null = null;
      let closed = false;

      const sendChunk = async () => {
        if (closed) return;
        try {
          const { size } = await stat(file);
          if (size <= offset) return;
          const handle = await open(file, "r");
          const buf = Buffer.alloc(size - offset);
          await handle.read(buf, 0, buf.length, offset);
          await handle.close();
          offset = size;
          for (const line of buf.toString("utf-8").split("\n")) {
            if (line.length === 0) continue;
            controller.enqueue(`data: ${JSON.stringify({ line })}\n\n`);
          }
        } catch {
          // file briefly missing/rotating; ignore and retry on next event
        }
      };

      try {
        watcher = watch(join(RIKU_ROOT, "logs", safeApp), () => {
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
