import { CHALLENGE_SECONDS, SESSION_SECONDS } from "@/lib/server/cookie-options";
import { open, seal, sign, verify } from "@/lib/server/seal";

/**
 * What the site's cookies hold. The session cookie carries the cloud's web session token sealed (AES-256-GCM), so
 * the browser keeps an opaque value it cannot use against the API; it also carries its own expiry, so a cookie kept
 * past its 12 hours is refused here even if the browser still sends it.
 */
export function sealSession(key: Buffer, token: string, now: number): string {
  return seal(key, JSON.stringify({ token, expires: now + SESSION_SECONDS * 1000 }));
}

export function openSession(key: Buffer, value: string | undefined, now: number): string | null {
  const plaintext = value === undefined ? null : open(key, value);
  if (plaintext === null) {
    return null;
  }
  const session = JSON.parse(plaintext) as { token?: unknown; expires?: unknown };
  const fresh = typeof session.expires === "number" && session.expires > now;
  return fresh && typeof session.token === "string" ? session.token : null;
}

/** *Check your email* after sign-up: the address, when the last email went out and until when the cloud refuses more. */
export interface SignupFlow {
  readonly email: string;
  readonly sentAt: number;
  /** Set when the last send was a resend, for *Sent again at …*. */
  readonly resent: boolean;
  /** Set when the cloud answered `rate_limited` to a resend. */
  readonly limitedUntil?: number;
}

/** *Check your email* after *Send reset link*: only the address. */
export interface ForgotFlow {
  readonly email: string;
}

/**
 * Flow cookies carry their own expiry inside the signed value, so one kept or replayed past it is refused even if a
 * browser still sends it. Within its life a replayed older value still reads (no state is kept here); the cloud's own
 * limit of 5 emails an hour per address is what bounds resends, the 60 s cooldown only paces the page.
 */
export function signFlow(key: Buffer, flow: SignupFlow | ForgotFlow, now: number, lifetimeSeconds: number): string {
  return sign(key, JSON.stringify({ ...flow, expires: now + lifetimeSeconds * 1000 }));
}

function readJson(key: Buffer, value: string | undefined, now: number): Record<string, unknown> | null {
  const payload = value === undefined ? null : verify(key, value);
  if (payload === null) {
    return null;
  }
  const flow = JSON.parse(payload) as Record<string, unknown>;
  return typeof flow.expires === "number" && flow.expires > now ? flow : null;
}

export function readSignupFlow(key: Buffer, value: string | undefined, now: number): SignupFlow | null {
  const flow = readJson(key, value, now);
  if (typeof flow?.email !== "string" || typeof flow.sentAt !== "number" || typeof flow.resent !== "boolean") {
    return null;
  }
  const limited = typeof flow.limitedUntil === "number" ? { limitedUntil: flow.limitedUntil } : {};
  return { email: flow.email, sentAt: flow.sentAt, resent: flow.resent, ...limited };
}

export function readForgotFlow(key: Buffer, value: string | undefined, now: number): ForgotFlow | null {
  const flow = readJson(key, value, now);
  return typeof flow?.email === "string" ? { email: flow.email } : null;
}

/**
 * The sign-in waiting for its code (`/signin/code`): the cloud's challenge from the password step, sealed in this
 * browser's cookie for the five minutes the cloud gives it, with the email it is for and where to go after.
 */
export interface Challenge {
  readonly challenge: string;
  readonly email: string;
  /** `/authorize` when a team server's admin asked who this is, as the password step was given it. */
  readonly next: string | null;
}

export function sealChallenge(key: Buffer, challenge: Challenge, now: number): string {
  return seal(key, JSON.stringify({ ...challenge, expires: now + CHALLENGE_SECONDS * 1000 }));
}

export function openChallenge(key: Buffer, value: string | undefined, now: number): Challenge | null {
  const plaintext = value === undefined ? null : open(key, value);
  if (plaintext === null) {
    return null;
  }
  const kept = JSON.parse(plaintext) as Record<string, unknown>;
  const { challenge, email, next, expires } = kept;
  if (typeof expires !== "number" || expires <= now || typeof challenge !== "string" || typeof email !== "string") {
    return null;
  }
  return { challenge, email, next: typeof next === "string" ? next : null };
}
