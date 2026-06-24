import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import { homedir } from "node:os";
import { X509Certificate } from "node:crypto";

export const runtime = "nodejs";

const NGINX_DIR = join(process.env.RIKU_ROOT ?? join(homedir(), ".riku"), "nginx");

type NetworkEntry = {
  app: string;
  serverName: string | null;
  upstream: string | null;
  tlsExpiry: string | null;
};

// riku has no runtime API for vhost/TLS state; we read the same generated
// files nginx itself uses (see templates/nginx*.conf.tera, src/nginx.rs).
function parseServerName(conf: string): string | null {
  const m = conf.match(/server_name\s+([^;]+);/);
  return m ? m[1].trim() : null;
}

function parseUpstream(conf: string): string | null {
  const m = conf.match(/proxy_pass\s+(http:\/\/[^;]+);/);
  return m ? m[1].trim() : null;
}

async function readCertExpiry(app: string): Promise<string | null> {
  for (const name of [`${app}.fullchain.crt`, `${app}.crt`]) {
    try {
      const pem = await readFile(join(NGINX_DIR, name), "utf-8");
      const cert = new X509Certificate(pem);
      return cert.validTo; // real expiry parsed from the actual cert, not guessed
    } catch {
      continue;
    }
  }
  return null;
}

export async function GET() {
  let files: string[];
  try {
    files = await readdir(NGINX_DIR);
  } catch {
    return Response.json({ apps: [] satisfies NetworkEntry[] });
  }

  const confFiles = files.filter((f) => f.endsWith(".conf") && f !== "riku.conf");

  const apps: NetworkEntry[] = await Promise.all(
    confFiles.map(async (file) => {
      const app = file.replace(/\.conf$/, "");
      const conf = await readFile(join(NGINX_DIR, file), "utf-8").catch(() => "");
      return {
        app,
        serverName: parseServerName(conf),
        upstream: parseUpstream(conf),
        tlsExpiry: await readCertExpiry(app),
      };
    }),
  );

  return Response.json({ apps });
}
