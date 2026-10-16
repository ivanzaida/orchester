import type { ServerView } from "@/lib/authorize";
import { call, type ApiResult, type CallOptions } from "@/lib/server/api";
import { record } from "@/lib/server/cloud";

/**
 * The cloud's team server registry and invites in flight (`docs/cloud-api.md`): what `/authorize` and
 * `/invites/deliver` call, with the shapes of `crates/api` `cloud.rs`.
 */

/** What the admin's callback gets back: the identity token, the key set, and for a link the root key the server pins. */
export interface Answer {
  readonly identityToken: string;
  readonly keySet: string;
  readonly rootKey?: string;
}

function text(value: unknown): value is string {
  return typeof value === "string";
}

function readServer(json: unknown): ServerView | null {
  const { name, address, check_code: checkCode } = record(json) ?? {};
  if (!text(name) || !text(address) || (checkCode !== null && checkCode !== undefined && !text(checkCode))) {
    return null;
  }
  return { name, address, checkCode: checkCode ?? null };
}

/** `GET /servers/{id}`: a server linked to the cloud; `404` when it is not. */
export function linkedServer(id: string, options: CallOptions): Promise<ApiResult<ServerView>> {
  const read = (json: unknown) => readServer(record(json)?.server);
  return call("GET", `/servers/${encodeURIComponent(id)}`, { expected: 200, read }, options);
}

function readLinked(json: unknown): Answer | null {
  const body = record(json);
  const { identity_token: identityToken, key_set: keySet, root_key: rootKey } = body ?? {};
  return text(identityToken) && text(keySet) && text(rootKey) ? { identityToken, keySet, rootKey } : null;
}

/** `POST /servers`: links the server that signed `statement` to the signed-in account. */
export function linkServer(statement: string, options: CallOptions): Promise<ApiResult<Answer>> {
  return call("POST", "/servers", { body: { link: statement }, expected: 201, read: readLinked }, options);
}

/** `POST /identity-tokens` with purpose `admin`: a sign-in to that server's admin. */
export function adminToken(server: string, options: CallOptions): Promise<ApiResult<string>> {
  const read = (json: unknown) => {
    const token = record(json)?.identity_token;
    return text(token) ? token : null;
  };
  return call("POST", "/identity-tokens", { body: { audience: server, purpose: "admin" }, expected: 201, read }, options);
}

/** `GET /keys`: the newest key set, which a sign-in carries to a server that may not have it. */
export function keySet(options: CallOptions): Promise<ApiResult<string>> {
  const read = (json: unknown) => {
    const set = record(json)?.key_set;
    return text(set) ? set : null;
  };
  return call("GET", "/keys", { expected: 200, read }, options);
}

/** The team server an invite is from, as the cloud registered it. */
export interface InvitingServer {
  readonly name: string;
  readonly address: string;
}

function readInviting(json: unknown): InvitingServer | null {
  const { name, address } = record(record(json)?.server) ?? {};
  return text(name) && text(address) ? { name, address } : null;
}

/**
 * `POST /invites`: an invite a team server signed, carried here by its inviter's browser. The answer names the
 * server as the cloud registered it: the only place the page leads back to.
 */
export function deliverInvite(invite: string, options: CallOptions): Promise<ApiResult<InvitingServer>> {
  return call("POST", "/invites", { body: { invite }, expected: 202, read: readInviting }, options);
}
