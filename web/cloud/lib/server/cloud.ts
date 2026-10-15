import { call, nothing, type ApiResult, type CallOptions } from "@/lib/server/api";

/**
 * The cloud's endpoints the web front uses, with the shapes of `crates/api` `cloud.rs`. Two are assumed, not yet in
 * the backend (`.tmp/cloud-web-run/report.md` lists them for the coordinator): `POST /auth/confirm/resend` and
 * `POST /auth/links/inspect`.
 */

export interface Account {
  readonly email: string;
  readonly displayName: string;
}

export interface SignedIn {
  readonly token: string;
  readonly account: Account;
}

export type LinkPurpose = "confirm" | "reset";

export interface DeviceInfo {
  readonly name: string;
  readonly platform: "web";
}

export function record(json: unknown): Record<string, unknown> | null {
  return typeof json === "object" && json !== null ? (json as Record<string, unknown>) : null;
}

export function readAccount(json: unknown): Account | null {
  const account = record(json);
  const email = account?.email;
  const displayName = account?.display_name;
  return typeof email === "string" && typeof displayName === "string" ? { email, displayName } : null;
}

function readSignedIn(json: unknown): SignedIn | null {
  const body = record(json);
  const token = body?.token;
  const account = readAccount(body?.account);
  return typeof token === "string" && account !== null ? { token, account } : null;
}

function readEmail(json: unknown): string | null {
  const email = record(json)?.email;
  return typeof email === "string" ? email : null;
}

export function signUp(
  input: { email: string; password: string; displayName: string },
  options: CallOptions,
): Promise<ApiResult<true>> {
  const body = { email: input.email, password: input.password, display_name: input.displayName };
  return call("POST", "/auth/signup", { body, expected: 202, read: nothing }, options);
}

/** Assumed: sends a new confirmation link to an unconfirmed account; `202` for any email, like sign-up. */
export function resendConfirmation(email: string, options: CallOptions): Promise<ApiResult<true>> {
  return call("POST", "/auth/confirm/resend", { body: { email }, expected: 202, read: nothing }, options);
}

/**
 * Assumed: the address a link is for, without using the link up; `422 invalid_link` when it is unknown, used,
 * expired or for the other purpose.
 */
export function inspectLink(token: string, purpose: LinkPurpose, options: CallOptions): Promise<ApiResult<string>> {
  const body = { link_token: token, purpose };
  return call("POST", "/auth/links/inspect", { body, expected: 200, read: readEmail }, options);
}

/**
 * Confirms with the password chosen at sign-up (decision 65). Any failure, a wrong password or a link that is unknown,
 * used or expired, is one `401 confirm_failed`.
 */
export function confirmEmail(token: string, password: string, options: CallOptions): Promise<ApiResult<true>> {
  const body = { link_token: token, password };
  return call("POST", "/auth/confirm", { body, expected: 204, read: nothing }, options);
}

export function signIn(
  input: { email: string; password: string; device: DeviceInfo },
  options: CallOptions,
): Promise<ApiResult<SignedIn>> {
  return call("POST", "/auth/sessions", { body: input, expected: 201, read: readSignedIn }, options);
}

export function signOut(options: CallOptions): Promise<ApiResult<true>> {
  return call("DELETE", "/auth/sessions/current", { expected: 204, read: nothing }, options);
}

export function me(options: CallOptions): Promise<ApiResult<Account>> {
  return call("GET", "/me", { expected: 200, read: (json) => readAccount(record(json)?.account) }, options);
}

export function requestReset(email: string, options: CallOptions): Promise<ApiResult<true>> {
  return call("POST", "/auth/reset", { body: { email }, expected: 202, read: nothing }, options);
}

export function finishReset(token: string, password: string, options: CallOptions): Promise<ApiResult<true>> {
  const body = { link_token: token, new_password: password };
  return call("POST", "/auth/reset/finish", { body, expected: 204, read: nothing }, options);
}
