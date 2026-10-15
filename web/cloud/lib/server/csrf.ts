import { createHash } from "node:crypto";

import { mac, sameText } from "@/lib/server/seal";

/**
 * CSRF protection for every form (`docs/architecture/server.md`, "CSRF"): the request's `Origin` must be the site's
 * own, and the form's hidden token must be the HMAC of the visitor's session cookie, or before sign-in of the
 * pre-session cookie (a signed double submit). `SameSite=Lax` stops cross-site posts as well; the checks do not rely
 * on each other.
 */
export interface CsrfCookies {
  readonly session?: string | undefined;
  readonly presession?: string | undefined;
}

function binding(cookies: CsrfCookies): string | null {
  if (cookies.session !== undefined) {
    return `session:${createHash("sha256").update(cookies.session).digest("base64url")}`;
  }
  return cookies.presession === undefined ? null : `pre:${cookies.presession}`;
}

/** The token a form rendered for these cookies carries, or `null` when the visitor has neither cookie yet. */
export function csrfToken(key: Buffer, cookies: CsrfCookies): string | null {
  const bound = binding(cookies);
  return bound === null ? null : mac(key, bound);
}

export function tokenMatches(key: Buffer, cookies: CsrfCookies, submitted: unknown): boolean {
  const expected = csrfToken(key, cookies);
  return expected !== null && typeof submitted === "string" && sameText(expected, submitted);
}

/** A missing `Origin` is refused too: every browser that can send this form sends one with a `POST`. */
export function originMatches(origin: string | null, publicOrigin: string): boolean {
  return origin !== null && origin === publicOrigin;
}
