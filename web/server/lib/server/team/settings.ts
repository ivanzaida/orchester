import type { CloudLink, Settings } from "@/lib/model";
import { call, type ApiResult, type CallOptions } from "@/lib/server/api";
import {
  numberOrNull,
  readCertificateOrNull,
  readUserRefOrNull,
  record,
  text,
  textOrNull,
} from "@/lib/server/team/read";

/** The server's settings and its link to Orchester Cloud, for admins (contract, "Server API"). */

function readSettings(json: unknown): Settings | null {
  const body = record(json) ?? {};
  const { join_approval: joinApproval, name } = record(body.settings) ?? {};
  const checkCode = textOrNull(body.check_code);
  const certificate = readCertificateOrNull(body.certificate);
  if (typeof joinApproval !== "boolean" || !text(name) || checkCode === undefined || certificate === undefined) {
    return null;
  }
  return { joinApproval, name, checkCode, certificate };
}

export function settings(options: CallOptions): Promise<ApiResult<Settings>> {
  return call("GET", "/server/settings", { expected: 200, read: readSettings }, options);
}

export function updateSettings(changes: { joinApproval?: boolean; name?: string }, options: CallOptions): Promise<ApiResult<Settings>> {
  const body = {
    ...(changes.joinApproval === undefined ? {} : { join_approval: changes.joinApproval }),
    ...(changes.name === undefined ? {} : { name: changes.name }),
  };
  return call("PATCH", "/server/settings", { body, expected: 200, read: readSettings }, options);
}

function readTimes(body: Record<string, unknown>) {
  const keySetIssuedAt = numberOrNull(body.key_set_issued_at);
  const keySetExpiresAt = numberOrNull(body.key_set_expires_at);
  const linkedAt = numberOrNull(body.linked_at);
  if (keySetIssuedAt === undefined || keySetExpiresAt === undefined || linkedAt === undefined) {
    return null;
  }
  return { keySetIssuedAt, keySetExpiresAt, linkedAt };
}

function readCloudLink(json: unknown): CloudLink | null {
  const body = record(json) ?? {};
  const { server, accepting } = body;
  const rootFingerprint = textOrNull(body.root_fingerprint);
  const linkedBy = readUserRefOrNull(body.linked_by);
  const times = readTimes(body);
  if (!text(server) || typeof accepting !== "boolean" || rootFingerprint === undefined || linkedBy === undefined || times === null) {
    return null;
  }
  return { server, rootFingerprint, accepting, linkedBy, ...times };
}

export interface CloudAnswer {
  readonly cloud: CloudLink;
  /** The signed link statement, only after `link: "start"`. */
  readonly statement: string | null;
}

function readCloudAnswer(json: unknown): CloudAnswer | null {
  const body = record(json) ?? {};
  const cloud = readCloudLink(body.cloud);
  const statement = textOrNull(body.statement);
  return cloud !== null && statement !== undefined ? { cloud, statement } : null;
}

export function cloudLink(options: CallOptions): Promise<ApiResult<CloudAnswer>> {
  return call("GET", "/server/cloud", { expected: 200, read: readCloudAnswer }, options);
}

export type CloudChange =
  | { readonly link: "start" }
  | { readonly link: { readonly root_key: string; readonly key_set: string; readonly identity_token: string } }
  | { readonly accepting: boolean }
  | { readonly pin: string }
  | { readonly unlink: true };

export function changeCloud(change: CloudChange, options: CallOptions): Promise<ApiResult<CloudAnswer>> {
  return call("PATCH", "/server/cloud", { body: change, expected: 200, read: readCloudAnswer }, options);
}
