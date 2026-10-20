import type { Approval, ServerCloud, ServerInfo, User } from "@/lib/model";
import { call, isId, NOT_AN_ID, nothing, type ApiResult, type CallOptions } from "@/lib/server/api";
import {
  field,
  list,
  number,
  readCertificateOrNull,
  readUser,
  readUserRef,
  record,
  text,
  textOrNull,
} from "@/lib/server/team/read";

/** Signing in to the admin and the server's own description (contract, "Server API"). */

function readCloud(json: unknown): ServerCloud | null | undefined {
  if (json === null) {
    return null;
  }
  const { server, root_fingerprint: rootFingerprint, accepting } = record(json) ?? {};
  const fingerprint = textOrNull(rootFingerprint);
  if (!text(server) || fingerprint === undefined || typeof accepting !== "boolean") {
    return undefined;
  }
  return { server, rootFingerprint: fingerprint, accepting };
}

function readServer(json: unknown): ServerInfo | null {
  const body = record(json) ?? {};
  const { name, api_version: apiVersion, needs_setup: needsSetup, version } = body;
  const cloud = readCloud(body.cloud);
  const certificate = readCertificateOrNull(body.certificate);
  if (!text(name) || !number(apiVersion) || typeof needsSetup !== "boolean" || !text(version)) {
    return null;
  }
  if (cloud === undefined || certificate === undefined) {
    return null;
  }
  return { name, apiVersion, needsSetup, cloud, certificate, version };
}

export function serverInfo(options: CallOptions): Promise<ApiResult<ServerInfo>> {
  return call("GET", "/server", { expected: 200, read: readServer }, options);
}

/** `204` when the code is the open setup code; nothing is used up. */
export function checkSetupCode(code: string, options: CallOptions): Promise<ApiResult<true>> {
  return call("POST", "/auth/setup", { body: { code }, expected: 204, read: nothing }, options);
}

export interface Joined {
  readonly token: string;
  readonly user: User;
  /** The session signed in with two-factor authentication, so it may make invites; `null` from a server before it. */
  readonly mfa: boolean | null;
}

function readJoined(json: unknown): Joined | null {
  const token = record(json)?.token;
  const user = field(json, "user", readUser);
  const mfa = record(record(json)?.session)?.mfa;
  return text(token) && user !== null ? { token, user, mfa: typeof mfa === "boolean" ? mfa : null } : null;
}

export interface CodeSignIn {
  readonly code: string;
  readonly device: string;
  /** Only with the setup code, which makes the first admin. */
  readonly account?: { readonly username: string; readonly displayName: string };
}

export function signInWithCode(input: CodeSignIn, options: CallOptions): Promise<ApiResult<Joined>> {
  const account = input.account === undefined ? {} : { username: input.account.username, display_name: input.account.displayName };
  const body = { code: input.code, device: input.device, kind: "admin_web", ...account };
  return call("POST", "/auth/code", { body, expected: 201, read: readJoined }, options);
}

export interface CloudSignIn {
  readonly identityToken: string;
  readonly keySet: string | null;
  readonly device: string;
}

export function signInWithCloud(input: CloudSignIn, options: CallOptions): Promise<ApiResult<Joined>> {
  const keySet = input.keySet === null ? {} : { key_set: input.keySet };
  const body = { identity_token: input.identityToken, ...keySet, device: input.device, kind: "admin_web" };
  return call("POST", "/auth/cloud", { body, expected: 201, read: readJoined }, options);
}

export function me(options: CallOptions): Promise<ApiResult<User>> {
  return call("GET", "/auth/me", { expected: 200, read: (json) => field(json, "user", readUser) }, options);
}

export function signOut(options: CallOptions): Promise<ApiResult<true>> {
  return call("DELETE", "/auth/sessions/current", { expected: 204, read: nothing }, options);
}

function readApproval(json: unknown): Approval | null {
  const body = record(json) ?? {};
  const { id, device, address, requested_at: requestedAt, expires_at: expiresAt } = body;
  const user = readUserRef(body.user);
  const from = address === undefined ? null : textOrNull(address);
  if (!text(id) || user === null || !text(device) || from === undefined || !number(requestedAt) || !number(expiresAt)) {
    return null;
  }
  return { id, user, device, address: from, requestedAt, expiresAt };
}

/** The computers waiting for join approval; admins see every one. */
export function approvals(options: CallOptions): Promise<ApiResult<Approval[]>> {
  return call("GET", "/auth/approvals", { expected: 200, read: (json) => list(json, "approvals", readApproval) }, options);
}

export function decideApproval(id: string, approve: boolean, options: CallOptions): Promise<ApiResult<true>> {
  if (!isId(id)) {
    return Promise.resolve(NOT_AN_ID);
  }
  const path = `/auth/approvals/${encodeURIComponent(id)}`;
  return call("POST", path, { body: { approve }, expected: 200, read: () => true }, options);
}
