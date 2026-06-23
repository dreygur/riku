import type { NextConfig } from "next";

// Conservative security headers applied to every route.
//
// Tradeoff note: `style-src` and `script-src` include `'unsafe-inline'`
// because Next.js + Tailwind inject inline <style> tags and Next's runtime
// emits inline bootstrap <script> blocks. Without `'unsafe-inline'` the app
// would break. This is a deliberate, documented relaxation; everything else
// is locked down to `'self'`, and `frame-ancestors 'none'` (plus
// X-Frame-Options: DENY) prevents clickjacking via framing.
const securityHeaders = [
  { key: "X-Frame-Options", value: "DENY" },
  { key: "X-Content-Type-Options", value: "nosniff" },
  { key: "Referrer-Policy", value: "no-referrer" },
  {
    key: "Content-Security-Policy",
    value: [
      "default-src 'self'",
      "img-src 'self' data:",
      "style-src 'self' 'unsafe-inline'",
      "script-src 'self' 'unsafe-inline'",
      "connect-src 'self'",
      "frame-ancestors 'none'",
      "base-uri 'self'",
      "form-action 'self'",
    ].join("; "),
  },
];

const nextConfig: NextConfig = {
  async headers() {
    return [
      {
        source: "/:path*",
        headers: securityHeaders,
      },
    ];
  },
};

export default nextConfig;
