import { watch, type FSWatcher } from "node:fs";
import { open, stat } from "node:fs/promises";
import { join } from "node:path";
import { homedir } from "node:os";

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
  const file = deployLogPath(app);

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
        watcher = watch(join(RIKU_ROOT, "logs", app), () => {
          sendChunk();
        });
      } catch {
        // app log dir doesn't exist yet; client gets nothing until it does
      }

      const heartbeat = setInterval(() => {
        if (closed) return;
        controller.enqueue(`: heartbeat\n\n`);
      }, 15_000);

      req.signal.addEventListener("abort", () => {
        closed = true;
        watcher?.close();
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
