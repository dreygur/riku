import { NextResponse } from "next/server";
import { createSessionToken, SESSION_COOKIE, SESSION_TTL_MS } from "@/lib/auth";
import { verifyPassword } from "@/lib/password-hash";

export const dynamic = "force-dynamic";

function isHttps(req: Request): boolean {
  const proto = req.headers.get("x-forwarded-proto");
  if (proto) return proto === "https";
  return new URL(req.url).protocol === "https:";
}

export async function POST(req: Request) {
  const storedHash = process.env.RIKU_DASHBOARD_PASSWORD_HASH;
  if (!storedHash) {
    return NextResponse.json({ error: "auth not configured" }, { status: 500 });
  }

  const body = await req.json().catch(() => null);
  const attempt = typeof body?.password === "string" ? body.password : "";

  if (!(await verifyPassword(attempt, storedHash))) {
    return NextResponse.json({ error: "incorrect password" }, { status: 401 });
  }

  // The session HMAC key is derived from the stored hash, not the plaintext
  // password — the plaintext never persists past this request.
  const res = NextResponse.json({ ok: true });
  res.cookies.set(SESSION_COOKIE, await createSessionToken(storedHash), {
    httpOnly: true,
    secure: isHttps(req),
    sameSite: "strict",
    path: "/",
    maxAge: SESSION_TTL_MS / 1000,
  });
  return res;
}
