import { NextResponse, type NextRequest } from "next/server";

import { config as adminConfig } from "@/lib/server/config";
import { PRESESSION_COOKIE, PRESESSION_SECONDS, cookieOptions } from "@/lib/server/cookie-options";
import { HTTPS_ORIGIN_HEADER, needsHttps, originOf, placeOf } from "@/lib/server/place";
import { newPresession, presessionLive } from "@/lib/server/presession";
import { deriveKey, sign } from "@/lib/server/seal";

/**
 * Runs before every page: a fresh CSP nonce (Next.js puts it on its own scripts), the security headers, a pre-session
 * cookie for the CSRF token of people who are not signed in yet, and the HTTPS rule: over plain HTTP anywhere but the
 * server itself, every admin path answers *This page needs HTTPS* and no form works.
 */
function contentSecurityPolicy(nonce: string, development: boolean): string {
  // Development needs `eval` for React's error overlay and inline styles for Next's dev indicator; production neither.
  const script = `'self' 'nonce-${nonce}' 'strict-dynamic'${development ? " 'unsafe-eval'" : ""}`;
  const style = development ? "'self' 'unsafe-inline'" : `'self' 'nonce-${nonce}'`;
  // Forms go to the admin itself, and to Orchester Cloud: the sign-in and link trips, and an invite sent through
  // the browser when the server cannot reach the cloud.
  const cloud = adminConfig().cloudOrigin;
  return [
    "default-src 'self'",
    `script-src ${script}`,
    `style-src ${style}`,
    "img-src 'self'",
    "font-src 'self'",
    "connect-src 'self'",
    "object-src 'none'",
    "base-uri 'none'",
    `form-action 'self'${cloud === null ? "" : ` ${cloud}`}`,
    "frame-ancestors 'none'",
  ].join("; ");
}

function randomValue(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(32));
  return Buffer.from(bytes).toString("base64url");
}

const HTTPS_PAGE = "/admin/https";

/**
 * The HTTPS page for an admin path reached over plain HTTP. It names the address the browser used: a rewrite renders
 * with Next's own forwarded headers (Next reaches itself again through `localhost`), so the place travels in a
 * request header this proxy signs ([`HTTPS_ORIGIN_HEADER`]), never in an address a link could carry (security
 * review 1, minor 2).
 */
function target(request: NextRequest): { url: URL; origin: string } | null {
  const { pathname } = request.nextUrl;
  const admin = pathname === "/admin" || pathname.startsWith("/admin/");
  const place = placeOf(request.headers);
  if (!admin || pathname === HTTPS_PAGE || !needsHttps(place)) {
    return null;
  }
  return { url: new URL(HTTPS_PAGE, request.url), origin: originOf(place) };
}

export function proxy(request: NextRequest): NextResponse {
  const nonce = randomValue();
  const policy = contentSecurityPolicy(nonce, process.env.NODE_ENV === "development");
  const now = Date.now();
  const sent = request.cookies.get(PRESESSION_COOKIE)?.value;
  // A pre-session past the expiry it carries is replaced, like a missing one.
  const existing = sent !== undefined && presessionLive(sent, now) ? sent : undefined;
  const presession = existing ?? newPresession(now);
  // The page rendered for this request reads the cookie from the request, so it gets it even on the first visit.
  request.cookies.set(PRESESSION_COOKIE, presession);
  const requestHeaders = new Headers(request.headers);
  requestHeaders.set("content-security-policy", policy);
  requestHeaders.set("x-nonce", nonce);
  const rewrite = target(request);
  if (rewrite !== null) {
    // Signed, so the page tells this proxy's word from a client's own header of that name.
    requestHeaders.set(HTTPS_ORIGIN_HEADER, sign(deriveKey(adminConfig().secret, "place"), rewrite.origin));
  }
  const response = rewrite === null
    ? NextResponse.next({ request: { headers: requestHeaders } })
    : NextResponse.rewrite(rewrite.url, { request: { headers: requestHeaders } });
  response.headers.set("content-security-policy", policy);
  response.headers.set("cache-control", "no-store");
  // Set once and not renewed: a form older than the cookie answers *This form has expired*, honestly.
  if (existing === undefined) {
    response.cookies.set(PRESESSION_COOKIE, presession, cookieOptions(PRESESSION_SECONDS));
  }
  return response;
}

export const config = {
  matcher: [{ source: "/((?!_next/static|favicon.ico).*)" }],
};
