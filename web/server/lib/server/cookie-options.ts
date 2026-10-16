/**
 * The admin's cookies (contract: `.tmp/sa-run/contract.md`, "Admin app environment"). `__Host-` binds each to this
 * host: `Secure`, `Path=/` and no `Domain`. All are `HttpOnly`; no script on the page reads them.
 */
export const SESSION_COOKIE = "__Host-orchester_admin";
/** Carries the CSRF binding before sign-in. */
export const PRESESSION_COOKIE = "__Host-orchester_admin_presession";
/** The `state` of a trip to Orchester Cloud's `/authorize`; the cloud comes back with a cross-site form post. */
export const STATE_COOKIE = "__Host-orchester_admin_state";
/** The setup code between setup steps 1 and 2, sealed. */
export const SETUP_COOKIE = "__Host-orchester_admin_setup";
/** Who signed in with the cloud but has nothing to manage here, for that page's sentence. */
export const NOTHING_COOKIE = "__Host-orchester_admin_nothing";
/**
 * What a trip posts to the cloud's `/authorize/start` (the link statement among it), sealed, between the trip route and
 * the page that posts it: it never goes into a URL.
 */
export const HANDOFF_COOKIE = "__Host-orchester_admin_handoff";

const MINUTE = 60;
const HOUR = 60 * MINUTE;

/** The admin web token lives 12 hours from sign-in on the server, not renewed. */
export const SESSION_SECONDS = 12 * HOUR;
/** Long enough to fill a form, short enough that a stolen value is of little use. */
export const PRESESSION_SECONDS = 2 * HOUR;
export const STATE_SECONDS = 10 * MINUTE;
export const SETUP_SECONDS = 15 * MINUTE;
export const NOTHING_SECONDS = 10 * MINUTE;
export const HANDOFF_SECONDS = 5 * MINUTE;

export interface CookieOptions {
  readonly httpOnly: true;
  readonly secure: true;
  readonly sameSite: "lax" | "none";
  readonly path: "/";
  readonly maxAge: number;
}

export function cookieOptions(maxAge: number): CookieOptions {
  return { httpOnly: true, secure: true, sameSite: "lax", path: "/", maxAge };
}

/** The trip's state cookie must come back with the cloud's cross-site form `POST`, which `Lax` cookies do not follow. */
export function crossSiteCookieOptions(maxAge: number): CookieOptions {
  return { httpOnly: true, secure: true, sameSite: "none", path: "/", maxAge };
}
