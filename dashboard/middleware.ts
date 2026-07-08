import { NextResponse, type NextRequest } from "next/server";
import { SESSION_COOKIE, verifySessionToken } from "@/lib/auth";

const PUBLIC_PATHS = ["/login", "/api/login"];

export async function middleware(req: NextRequest) {
  // No password configured: auth is off (matches the rest of riku's
  // philosophy of not breaking default/local usage when a feature's env
  // var is unset).
  const storedHash = process.env.RIKU_DASHBOARD_PASSWORD_HASH;
  if (!storedHash) return NextResponse.next();

  const { pathname } = req.nextUrl;
  if (PUBLIC_PATHS.some((p) => pathname === p || pathname.startsWith(p + "/"))) {
    return NextResponse.next();
  }

  // The HMAC key is derived from the stored hash string itself — verifying
  // a session here never needs the plaintext password (see lib/password-hash.ts).
  const ok = await verifySessionToken(req.cookies.get(SESSION_COOKIE)?.value, storedHash);
  if (ok) return NextResponse.next();

  if (pathname.startsWith("/api/")) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const url = req.nextUrl.clone();
  url.pathname = "/login";
  url.searchParams.set("from", pathname);
  return NextResponse.redirect(url);
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
