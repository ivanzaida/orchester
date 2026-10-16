import { cookies, headers } from "next/headers";

import { CSRF_FIELD } from "@/lib/form";
import type { CallOptions } from "@/lib/server/api";
import { config } from "@/lib/server/config";
import {
  HANDOFF_COOKIE,
  HANDOFF_SECONDS,
  NOTHING_COOKIE,
  NOTHING_SECONDS,
  PRESESSION_COOKIE,
  SESSION_COOKIE,
  SESSION_SECONDS,
  SETUP_COOKIE,
  SETUP_SECONDS,
  STATE_COOKIE,
  STATE_SECONDS,
  cookieOptions,
  crossSiteCookieOptions,
} from "@/lib/server/cookie-options";
import { csrfToken, originMatches, tokenMatches, type CsrfCookies } from "@/lib/server/csrf";
import { clientOf, originOf, placeOf, type Place } from "@/lib/server/place";
import { deriveKey, type KeyPurpose } from "@/lib/server/seal";
import {
  openHandoff,
  openSession,
  openSetup,
  openTrip,
  readNothing,
  sealHandoff,
  sealSession,
  sealSetup,
  sealTrip,
  signNothing,
  type AdminSession,
  type Handoff,
  type NothingFlow,
  type Trip,
} from "@/lib/server/session";

/** The request's visitor as pages, actions and route handlers see it: where they are, cookies, CSRF and the session. */

const key = (purpose: KeyPurpose) => deriveKey(config().secret, purpose);

export async function place(): Promise<Place> {
  return placeOf(await headers());
}

/** The admin's own origin as the browser sees it, e.g. `https://orchester.kontur.dev`. */
export async function ownOrigin(): Promise<string> {
  return originOf(await place());
}

async function csrfCookies(): Promise<CsrfCookies> {
  const jar = await cookies();
  return { session: jar.get(SESSION_COOKIE)?.value, presession: jar.get(PRESESSION_COOKIE)?.value };
}

/** The token a form rendered now must carry; `proxy.ts` gives every visitor a pre-session cookie first. */
export async function formToken(): Promise<string> {
  const token = csrfToken(key("csrf"), await csrfCookies());
  if (token === null) {
    throw new Error("no pre-session cookie: proxy.ts did not run for this request");
  }
  return token;
}

/** Whether a posted form came from this admin's own page for this visitor. */
export async function formIsGenuine(form: FormData): Promise<boolean> {
  const origin = (await headers()).get("origin");
  return originMatches(origin, await ownOrigin()) && tokenMatches(key("csrf"), await csrfCookies(), form.get(CSRF_FIELD));
}

/** Call options without a session: where the browser is, for the server's limits and addresses. */
export async function anonymous(): Promise<CallOptions> {
  const list = await headers();
  return { place: placeOf(list), client: clientOf(list) };
}

export async function session(): Promise<AdminSession | null> {
  return openSession(key("session"), (await cookies()).get(SESSION_COOKIE)?.value, Date.now());
}

/** Call options for the signed-in person, or `null` without a live session. */
export async function signedIn(): Promise<CallOptions | null> {
  const current = await session();
  return current === null ? null : { ...(await anonymous()), token: current.token };
}

export async function startSession(current: AdminSession): Promise<void> {
  (await cookies()).set(SESSION_COOKIE, sealSession(key("session"), current, Date.now()), cookieOptions(SESSION_SECONDS));
}

/** Forgets a cookie. Only one that exists is deleted: deleting one makes Next.js render the page again. */
async function forget(name: string, sameSite: "lax" | "none" = "lax"): Promise<void> {
  const jar = await cookies();
  if (jar.has(name)) {
    jar.delete({ name, ...(sameSite === "none" ? crossSiteCookieOptions(0) : cookieOptions(0)) });
  }
}

export async function endSession(): Promise<void> {
  await forget(SESSION_COOKIE);
}

export async function setupCode(): Promise<string | null> {
  return openSetup(key("setup"), (await cookies()).get(SETUP_COOKIE)?.value, Date.now());
}

export async function keepSetupCode(code: string): Promise<void> {
  (await cookies()).set(SETUP_COOKIE, sealSetup(key("setup"), code, Date.now()), cookieOptions(SETUP_SECONDS));
}

export async function forgetSetupCode(): Promise<void> {
  await forget(SETUP_COOKIE);
}

export async function trip(): Promise<Trip | null> {
  return openTrip(key("state"), (await cookies()).get(STATE_COOKIE)?.value, Date.now());
}

export async function startTrip(value: Trip): Promise<void> {
  (await cookies()).set(STATE_COOKIE, sealTrip(key("state"), value, Date.now()), crossSiteCookieOptions(STATE_SECONDS));
}

export async function endTrip(): Promise<void> {
  await forget(STATE_COOKIE, "none");
}

export async function handoff(): Promise<Handoff | null> {
  return openHandoff(key("handoff"), (await cookies()).get(HANDOFF_COOKIE)?.value, Date.now());
}

export async function keepHandoff(value: Handoff): Promise<void> {
  (await cookies()).set(HANDOFF_COOKIE, sealHandoff(key("handoff"), value, Date.now()), cookieOptions(HANDOFF_SECONDS));
}

export async function nothingFlow(): Promise<NothingFlow | null> {
  return readNothing(key("flow"), (await cookies()).get(NOTHING_COOKIE)?.value, Date.now());
}

export async function setNothingFlow(flow: NothingFlow): Promise<void> {
  (await cookies()).set(NOTHING_COOKIE, signNothing(key("flow"), flow, Date.now()), cookieOptions(NOTHING_SECONDS));
}

export async function endNothingFlow(): Promise<void> {
  await forget(NOTHING_COOKIE);
}
