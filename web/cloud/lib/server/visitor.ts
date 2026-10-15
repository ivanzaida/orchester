import { cookies, headers } from "next/headers";

import { CSRF_FIELD } from "@/lib/form";
import type { CallOptions } from "@/lib/server/api";
import { config } from "@/lib/server/config";
import {
  FORGOT_FLOW_COOKIE,
  FORGOT_FLOW_SECONDS,
  PRESESSION_COOKIE,
  SESSION_COOKIE,
  SESSION_SECONDS,
  SIGNUP_FLOW_COOKIE,
  SIGNUP_FLOW_SECONDS,
  cookieOptions,
} from "@/lib/server/cookie-options";
import { csrfToken, originMatches, tokenMatches, type CsrfCookies } from "@/lib/server/csrf";
import { deriveKey } from "@/lib/server/seal";
import {
  openSession,
  readForgotFlow,
  readSignupFlow,
  sealSession,
  signFlow,
  type ForgotFlow,
  type SignupFlow,
} from "@/lib/server/session";

/** The request's visitor as pages and actions see it: cookies, CSRF, the session and the address. */

const key = (purpose: "csrf" | "session" | "flow") => deriveKey(config().secret, purpose);

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

/** Whether a posted form came from this site's own page for this visitor. */
export async function formIsGenuine(form: FormData): Promise<boolean> {
  const origin = (await headers()).get("origin");
  return originMatches(origin, config().publicOrigin) && tokenMatches(key("csrf"), await csrfCookies(), form.get(CSRF_FIELD));
}

/**
 * The visitor's address for the cloud's per-address limits: the last `X-Forwarded-For` entry, the one the site's own
 * proxy (Caddy) appended. Without a proxy (development) there is none and the cloud counts the web server's address.
 */
export async function clientAddress(): Promise<string | undefined> {
  const forwarded = (await headers()).get("x-forwarded-for");
  const last = forwarded?.split(",").at(-1)?.trim();
  return last === undefined || last === "" ? undefined : last;
}

export async function anonymous(): Promise<CallOptions> {
  return { client: await clientAddress() };
}

export async function sessionToken(): Promise<string | null> {
  return openSession(key("session"), (await cookies()).get(SESSION_COOKIE)?.value, Date.now());
}

/** Call options for the signed-in visitor, or `null` without a live session. */
export async function signedIn(): Promise<CallOptions | null> {
  const token = await sessionToken();
  return token === null ? null : { client: await clientAddress(), token };
}

export async function startSession(token: string): Promise<void> {
  const jar = await cookies();
  jar.set(SESSION_COOKIE, sealSession(key("session"), token, Date.now()), cookieOptions(SESSION_SECONDS));
}

/** Forgets the session. Only a cookie that exists is deleted: deleting one makes Next.js render the page again. */
export async function endSession(): Promise<void> {
  const jar = await cookies();
  if (jar.has(SESSION_COOKIE)) {
    jar.delete({ name: SESSION_COOKIE, ...cookieOptions(0) });
  }
}

export async function signupFlow(): Promise<SignupFlow | null> {
  return readSignupFlow(key("flow"), (await cookies()).get(SIGNUP_FLOW_COOKIE)?.value);
}

export async function setSignupFlow(flow: SignupFlow): Promise<void> {
  (await cookies()).set(SIGNUP_FLOW_COOKIE, signFlow(key("flow"), flow), cookieOptions(SIGNUP_FLOW_SECONDS));
}

export async function forgotFlow(): Promise<ForgotFlow | null> {
  return readForgotFlow(key("flow"), (await cookies()).get(FORGOT_FLOW_COOKIE)?.value);
}

export async function setForgotFlow(flow: ForgotFlow): Promise<void> {
  (await cookies()).set(FORGOT_FLOW_COOKIE, signFlow(key("flow"), flow), cookieOptions(FORGOT_FLOW_SECONDS));
}
