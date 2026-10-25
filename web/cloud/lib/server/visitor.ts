import { createHash } from "node:crypto";

import { cookies, headers } from "next/headers";

import { CSRF_FIELD } from "@/lib/form";
import { redirect } from "next/navigation";

import type { ApiFailure, CallOptions } from "@/lib/server/api";
import { config } from "@/lib/server/config";
import {
  DELIVERED_COOKIE,
  DELIVERED_SECONDS,
  AUTHORIZE_COOKIE,
  AUTHORIZE_SECONDS,
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
import { readRequest, requestParams, type AuthorizeRequest } from "@/lib/authorize";
import { deriveKey, open, sameText, seal, sign, verify } from "@/lib/server/seal";
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

const key = (purpose: "csrf" | "session" | "flow" | "authorize") => deriveKey(config().secret, purpose);

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

/**
 * Whether a posted form came from this site's own page for this visitor, on any of the site's origins: the CSRF token
 * binds the cookies of the host the form was posted to, which are that host's alone.
 */
export async function formIsGenuine(form: FormData): Promise<boolean> {
  const origin = (await headers()).get("origin");
  return originMatches(origin, config().siteOrigins) && tokenMatches(key("csrf"), await csrfCookies(), form.get(CSRF_FIELD));
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
  return readSignupFlow(key("flow"), (await cookies()).get(SIGNUP_FLOW_COOKIE)?.value, Date.now());
}

export async function setSignupFlow(flow: SignupFlow): Promise<void> {
  (await cookies()).set(SIGNUP_FLOW_COOKIE, signFlow(key("flow"), flow, Date.now(), SIGNUP_FLOW_SECONDS), cookieOptions(SIGNUP_FLOW_SECONDS));
}

export async function forgotFlow(): Promise<ForgotFlow | null> {
  return readForgotFlow(key("flow"), (await cookies()).get(FORGOT_FLOW_COOKIE)?.value, Date.now());
}

export async function setForgotFlow(flow: ForgotFlow): Promise<void> {
  (await cookies()).set(FORGOT_FLOW_COOKIE, signFlow(key("flow"), flow, Date.now(), FORGOT_FLOW_SECONDS), cookieOptions(FORGOT_FLOW_SECONDS));
}

/** Whether the cloud no longer knows the session (signed out elsewhere, a password reset, its 12 hours over). */
export function sessionEnded(failure: ApiFailure): boolean {
  return failure.kind === "refused" && failure.code === "unauthorized";
}

/**
 * Forgets a session the cloud ended (`401 unauthorized`) and sends the visitor to sign in, instead of reading it as
 * the cloud not answering. For server actions and route handlers, which may change cookies.
 */
export async function leaveIfEnded(failure: ApiFailure): Promise<void> {
  if (sessionEnded(failure)) {
    await endSession();
    redirect("/signin");
  }
}

/** Keeps a team server admin's request to `/authorize` for this browser, sealed, for 10 minutes. */
export async function keepAuthorizeRequest(request: AuthorizeRequest): Promise<void> {
  const value = seal(key("authorize"), JSON.stringify({ ...requestParams(request), expires: Date.now() + AUTHORIZE_SECONDS * 1000 }));
  (await cookies()).set(AUTHORIZE_COOKIE, value, cookieOptions(AUTHORIZE_SECONDS));
}

/**
 * The request a `/authorize` page was rendered for, as its confirm form carries it back: a digest of the request's own
 * fields. Confirming acts only when the cookie still holds that request, so one another site posted to
 * `/authorize/start` after the page loaded is not the one confirmed (security review 2, 1).
 */
export function authorizeDigest(request: AuthorizeRequest): string {
  const fields = Object.entries(requestParams(request)).sort(([a], [b]) => a.localeCompare(b));
  return createHash("sha256").update(JSON.stringify(fields)).digest("base64url");
}

export function sameAuthorizeRequest(request: AuthorizeRequest, shown: string): boolean {
  return sameText(authorizeDigest(request), shown);
}

/** The request this browser brought to `/authorize`, while it is fresh. */
export async function authorizeRequest(): Promise<AuthorizeRequest | null> {
  const value = (await cookies()).get(AUTHORIZE_COOKIE)?.value;
  const plaintext = value === undefined ? null : open(key("authorize"), value);
  if (plaintext === null) {
    return null;
  }
  const params = JSON.parse(plaintext) as Record<string, unknown>;
  if (typeof params.expires !== "number" || params.expires <= Date.now()) {
    return null;
  }
  const text = (name: string) => (typeof params[name] === "string" ? params[name] : undefined);
  return readRequest({ purpose: text("purpose"), server: text("server"), statement: text("statement"), state: text("state"),
    return: text("return") });
}

/** How a delivery from the inviter's browser went: its outcome, and the registered server's admin, if the cloud named it. */
export interface Delivered {
  readonly outcome: "sent" | "refused" | "unavailable";
  readonly back: string | null;
}

export async function keepDelivered(delivered: Delivered): Promise<void> {
  const value = sign(key("flow"), JSON.stringify({ ...delivered, expires: Date.now() + DELIVERED_SECONDS * 1000 }));
  (await cookies()).set(DELIVERED_COOKIE, value, cookieOptions(DELIVERED_SECONDS));
}

export async function delivered(): Promise<Delivered | null> {
  const value = (await cookies()).get(DELIVERED_COOKIE)?.value;
  const payload = value === undefined ? null : verify(key("flow"), value);
  if (payload === null) {
    return null;
  }
  const flow = JSON.parse(payload) as Record<string, unknown>;
  const outcome = flow.outcome;
  if (typeof flow.expires !== "number" || flow.expires <= Date.now()) {
    return null;
  }
  if (outcome !== "sent" && outcome !== "refused" && outcome !== "unavailable") {
    return null;
  }
  return { outcome, back: typeof flow.back === "string" ? flow.back : null };
}
