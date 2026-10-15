import { NextResponse, type NextRequest } from "next/server";

import { callbackUrl } from "@/lib/authorize";
import { PRESESSION_COOKIE, PRESESSION_SECONDS, cookieOptions } from "@/lib/server/cookie-options";
import { newPresession, presessionLive } from "@/lib/server/presession";

/**
 * Runs before every page: a fresh CSP nonce (Next.js puts it on its own scripts), the security headers, and a
 * pre-session cookie for the CSRF token of visitors who are not signed in yet.
 */
function contentSecurityPolicy(nonce: string, development: boolean, formTarget: string | null): string {
  // Development needs `eval` for React's error overlay and inline styles for Next's dev indicator; production neither.
  const script = `'self' 'nonce-${nonce}' 'strict-dynamic'${development ? " 'unsafe-eval'" : ""}`;
  const style = development ? "'self' 'unsafe-inline'" : `'self' 'nonce-${nonce}'`;
  return [
    "default-src 'self'",
    `script-src ${script}`,
    `style-src ${style}`,
    "img-src 'self'",
    "font-src 'self'",
    "connect-src 'self'",
    "object-src 'none'",
    "base-uri 'none'",
    // `/authorize` posts its answer to the team server admin's callback, and nowhere else.
    formTarget === null ? "form-action 'self'" : `form-action 'self' ${formTarget}`,
    "frame-ancestors 'none'",
  ].join("; ");
}

function randomValue(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(32));
  return Buffer.from(bytes).toString("base64url");
}

export function proxy(request: NextRequest): NextResponse {
  const nonce = randomValue();
  const callback = request.nextUrl.pathname === "/authorize" ? callbackUrl(request.nextUrl.searchParams.get("return") ?? undefined) : null;
  const policy = contentSecurityPolicy(nonce, process.env.NODE_ENV === "development", callback?.origin ?? null);
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
  const response = NextResponse.next({ request: { headers: requestHeaders } });
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
