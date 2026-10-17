/**
 * The fake cloud's two-factor authentication (`docs/cloud-api.md`, "Two-factor authentication"). It checks no time
 * steps: `123456` is the app's code whenever one is asked, `000000` and anything else is wrong, `999999` is refused as
 * too many tries, and each account has one recovery code, `abcde-fghjk`, which works once. `two.factor@example.com`
 * has it on; every other account starts with it off and turns it on with the password and `123456`.
 */
import type { Answer } from "./accounts.ts";

export const TWO_FACTOR = "two.factor@example.com";
export const APP_CODE = "123456";
export const RECOVERY_CODE = "abcde-fghjk";
const LIMITED_CODE = "999999";
const SECRET = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
const ISSUED = Date.UTC(2026, 8, 30, 9, 0);
const CODES = [RECOVERY_CODE, "bcdef-ghjkm", "cdefg-hjkmn", "defgh-jkmnp", "efghj-kmnpq", "fghjk-mnpqr", "ghjkm-npqrs",
  "hjkmn-pqrst", "jkmnp-qrstv", "kmnpq-rstvw"];

interface State {
  enabledAt: number | null;
  left: number;
  recoveryUsed: boolean;
  setup: boolean;
}

const states = new Map<string, State>();
/** Session tokens that signed in, stepped up or turned it on with a code. */
const withCode = new Set<string>();
const challenges = new Map<string, { email: string; device: unknown }>();
let issued = 0;

/** `POST /__fake/two-factor-on?email=`: turns it on for an account behind its sessions' back, as another device does. */
export function turnOnElsewhere(email: string): void {
  Object.assign(state(email), { enabledAt: Date.now(), left: 10, recoveryUsed: false, setup: false });
}

export function resetTwoFactor(): void {
  states.clear();
  withCode.clear();
  challenges.clear();
}

function state(email: string): State {
  let found = states.get(email);
  if (found === undefined) {
    const on = email === TWO_FACTOR;
    found = { enabledAt: on ? ISSUED : null, left: on ? 10 : 0, recoveryUsed: false, setup: false };
    states.set(email, found);
  }
  return found;
}

const refuse = (status: number, error: string, retryAfter?: number): Answer => ({
  status,
  body: { error, message: "fake" },
  ...(retryAfter === undefined ? {} : { retryAfter }),
});

/** Whether `code` is right for `email`'s authenticator; a recovery code is used up. */
function check(email: string, code: unknown, recovery = true): Answer | null {
  if (code === LIMITED_CODE) {
    return refuse(429, "rate_limited", 840);
  }
  const s = state(email);
  if (code === APP_CODE) {
    return null;
  }
  if (recovery && code === RECOVERY_CODE && !s.recoveryUsed && s.enabledAt !== null) {
    s.recoveryUsed = true;
    s.left -= 1;
    return null;
  }
  return refuse(401, "invalid_code");
}

export function twoFactorOf(email: string, token: string) {
  const s = state(email);
  return { enabled: s.enabledAt !== null, enabled_at: s.enabledAt, recovery_codes_left: s.left, this_session: withCode.has(token) };
}

/** The password step's answer for an account with it on: a challenge; `null` for one with it off. */
export function challengeFor(email: string, device: unknown): Answer | null {
  if (state(email).enabledAt === null) {
    return null;
  }
  issued += 1;
  const challenge = `orcm_fake_challenge_${issued}`;
  challenges.set(challenge, { email, device });
  return { status: 202, body: { challenge, expires_at: Date.now() + 5 * 60 * 1000 } };
}

/** `POST /auth/sessions/two-factor`; `open` signs the account in and answers the new token's answer. */
export function finishSignIn(body: Record<string, unknown>, open: (email: string, device: unknown) => Answer): Answer {
  const waiting = typeof body.challenge === "string" ? challenges.get(body.challenge) : undefined;
  if (waiting === undefined) {
    return refuse(401, "challenge_expired");
  }
  const refused = check(waiting.email, body.code);
  if (refused !== null) {
    return refused;
  }
  challenges.delete(body.challenge as string);
  const signedIn = open(waiting.email, waiting.device);
  const token = (signedIn.body as { token: string }).token;
  withCode.add(token);
  return { status: 201, body: { ...(signedIn.body as object), recovery_codes_left: state(waiting.email).left } };
}

/** The signed-in endpoints; `undefined` for a path that is not one of them. */
export function routeTwoFactor(key: string, body: Record<string, unknown>, who: { email: string; password: string; token: string }) {
  const s = state(who.email);
  const codes = (): Answer => ({ status: 200, body: { recovery_codes: CODES } });
  const handlers: Record<string, () => Answer> = {
    "POST /api/v1/me/two-factor": () => {
      if (s.enabledAt !== null) {
        return refuse(409, "conflict");
      }
      if (body.password !== who.password) {
        return refuse(401, "invalid_credentials");
      }
      s.setup = true;
      const uri = `otpauth://totp/localhost:${encodeURIComponent(who.email)}?secret=${SECRET}&issuer=localhost&algorithm=SHA1&digits=6&period=30`;
      return { status: 201, body: { secret: SECRET, uri } };
    },
    "POST /api/v1/me/two-factor/confirm": () => {
      if (!s.setup) {
        return refuse(404, "not_found");
      }
      const refused = check(who.email, body.code, false);
      if (refused !== null) {
        return refused;
      }
      Object.assign(s, { enabledAt: Date.now(), left: 10, recoveryUsed: false, setup: false });
      withCode.add(who.token);
      return codes();
    },
    "DELETE /api/v1/me/two-factor": () => {
      if (s.enabledAt === null) {
        return refuse(404, "not_found");
      }
      if (body.password !== who.password) {
        return refuse(401, "invalid_credentials");
      }
      const refused = check(who.email, body.code);
      if (refused !== null) {
        return refused;
      }
      Object.assign(s, { enabledAt: null, left: 0, recoveryUsed: false });
      withCode.clear();
      return { status: 204 };
    },
    "POST /api/v1/me/two-factor/recovery-codes": () => {
      const refused = s.enabledAt === null ? refuse(404, "not_found") : check(who.email, body.code);
      if (refused !== null) {
        return refused;
      }
      Object.assign(s, { left: 10, recoveryUsed: false });
      return codes();
    },
    "POST /api/v1/auth/sessions/current/two-factor": () => {
      const refused = s.enabledAt === null ? refuse(403, "mfa_required") : check(who.email, body.code);
      if (refused !== null) {
        return refused;
      }
      withCode.add(who.token);
      return { status: 200, body: { recovery_codes_left: s.left } };
    },
  };
  return handlers[key]?.();
}
