import { SESSION_SECONDS } from "@/lib/server/cookie-options";
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

export function signFlow(key: Buffer, flow: SignupFlow | ForgotFlow): string {
  return sign(key, JSON.stringify(flow));
}

function readJson(key: Buffer, value: string | undefined): Record<string, unknown> | null {
  const payload = value === undefined ? null : verify(key, value);
  return payload === null ? null : (JSON.parse(payload) as Record<string, unknown>);
}

export function readSignupFlow(key: Buffer, value: string | undefined): SignupFlow | null {
  const flow = readJson(key, value);
  if (typeof flow?.email !== "string" || typeof flow.sentAt !== "number" || typeof flow.resent !== "boolean") {
    return null;
  }
  const limited = typeof flow.limitedUntil === "number" ? { limitedUntil: flow.limitedUntil } : {};
  return { email: flow.email, sentAt: flow.sentAt, resent: flow.resent, ...limited };
}

export function readForgotFlow(key: Buffer, value: string | undefined): ForgotFlow | null {
  const flow = readJson(key, value);
  return typeof flow?.email === "string" ? { email: flow.email } : null;
}
