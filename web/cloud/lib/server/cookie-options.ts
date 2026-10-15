/**
 * The site's cookies (`docs/architecture/server.md`, "The cloud's web front"). `__Host-` binds each to this host:
 * `Secure`, `Path=/` and no `Domain`. All are `HttpOnly`; no script on the page reads them.
 */
export const SESSION_COOKIE = "__Host-orchester_session";
/** Carries the CSRF binding before sign-in. */
export const PRESESSION_COOKIE = "__Host-orchester_presession";
/** The address and send times shown on *Check your email* after sign-up or a resend. */
export const SIGNUP_FLOW_COOKIE = "__Host-orchester_signup";
/** The address shown after *Send reset link*. */
export const FORGOT_FLOW_COOKIE = "__Host-orchester_forgot";

const HOUR = 60 * 60;

/** A web session lives 12 hours (the cloud's `web` sessions). */
export const SESSION_SECONDS = 12 * HOUR;
/** Long enough to fill a form, short enough that a stolen value is of little use. */
export const PRESESSION_SECONDS = 2 * HOUR;
/** As long as a confirmation link works. */
export const SIGNUP_FLOW_SECONDS = 24 * HOUR;
/** As long as a reset link works. */
export const FORGOT_FLOW_SECONDS = HOUR;

export interface CookieOptions {
  readonly httpOnly: true;
  readonly secure: true;
  readonly sameSite: "lax";
  readonly path: "/";
  readonly maxAge: number;
}

export function cookieOptions(maxAge: number): CookieOptions {
  return { httpOnly: true, secure: true, sameSite: "lax", path: "/", maxAge };
}
