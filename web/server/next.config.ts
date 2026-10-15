import type { NextConfig } from "next";

// Headers for every answer, static files included; the page CSP with its nonce comes from `proxy.ts`.
// `strict-origin`, not `no-referrer`: a Referer never carries a path or query, while forms still send their `Origin`
// (under `no-referrer` browsers send `Origin: null`, and the CSRF origin check needs it).
const securityHeaders = [
  { key: "Referrer-Policy", value: "strict-origin" },
  { key: "X-Frame-Options", value: "DENY" },
  { key: "X-Content-Type-Options", value: "nosniff" },
];

const config: NextConfig = {
  output: "standalone",
  poweredByHeader: false,
  // The UI kit is TypeScript source in the workspace; Next compiles it with the app.
  transpilePackages: ["@orchester/ui"],
  reactStrictMode: true,
  async headers() {
    return [{ source: "/:path*", headers: securityHeaders }];
  },
};

export default config;
