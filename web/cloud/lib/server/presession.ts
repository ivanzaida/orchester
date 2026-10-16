import { PRESESSION_SECONDS } from "@/lib/server/cookie-options";

/**
 * The pre-session cookie that binds a form's CSRF token before sign-in: random bytes, then its expiry,
 * `<random>.<expires ms>`. The value is not signed itself; the CSRF token is the HMAC of the whole value, so a changed
 * expiry no longer matches its token, and a value past its expiry binds nothing even if a browser still sends it.
 * No `node:` imports: `proxy.ts` makes the value.
 */
export function newPresession(now: number): string {
  const bytes = crypto.getRandomValues(new Uint8Array(32));
  return `${Buffer.from(bytes).toString("base64url")}.${now + PRESESSION_SECONDS * 1000}`;
}

export function presessionLive(value: string, now: number): boolean {
  const [random, expires, extra] = value.split(".");
  return random !== undefined && random !== "" && extra === undefined && /^\d+$/.test(expires ?? "") && Number(expires) > now;
}
