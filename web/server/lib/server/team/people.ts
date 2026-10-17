import type { AdminBrowser, Device, JoinedVia, Person } from "@/lib/model";
import { call, isId, NOT_AN_ID, nothing, type ApiResult, type CallOptions } from "@/lib/server/api";
import { field, list, number, readUser, record, text } from "@/lib/server/team/read";

/** People and their computers, for admins (contract, "Server API"). */

function readPerson(json: unknown): Person | null {
  const user = readUser(json);
  const projects = record(json)?.projects;
  return user !== null && number(projects) ? { ...user, projects } : null;
}

export function people(options: CallOptions): Promise<ApiResult<Person[]>> {
  return call("GET", "/admin/users", { expected: 200, read: (json) => list(json, "users", readPerson) }, options);
}

export function updatePerson(id: string, changes: { admin?: boolean; disabled?: boolean }, options: CallOptions): Promise<ApiResult<true>> {
  if (!isId(id)) {
    return Promise.resolve(NOT_AN_ID);
  }
  const read = (json: unknown) => (field(json, "user", readUser) === null ? null : true);
  return call("PATCH", `/admin/users/${encodeURIComponent(id)}`, { body: changes, expected: 200, read }, options);
}

/** Ends the account's link to its Orchester Cloud identity (`DELETE /admin/users/{id}/cloud`); its computers stay. */
export function unlinkPerson(id: string, options: CallOptions): Promise<ApiResult<true>> {
  if (!isId(id)) {
    return Promise.resolve(NOT_AN_ID);
  }
  const read = (json: unknown) => (field(json, "user", readUser) === null ? null : true);
  return call("DELETE", `/admin/users/${encodeURIComponent(id)}/cloud`, { expected: 200, read }, options);
}

const VIAS = new Set<unknown>(["cloud", "code", "before_3"]);

function readDevice(json: unknown): Device | null {
  const body = record(json) ?? {};
  const { id, device, via, created_at: createdAt, last_used_at: lastUsedAt, expires_at: expiresAt } = body;
  const { id: userId, username, display_name: displayName } = record(body.user) ?? {};
  if (!text(id) || !text(device) || !VIAS.has(via) || !number(createdAt) || !number(lastUsedAt) || !number(expiresAt)) {
    return null;
  }
  if (!text(userId) || !text(username) || !text(displayName)) {
    return null;
  }
  return { id, device, user: { id: userId, username, displayName }, via: via as JoinedVia, createdAt, lastUsedAt, expiresAt };
}

/** Computers joined to the server (app device tokens), newest use first; admin browser sign-ins are not listed. */
export function devices(options: CallOptions): Promise<ApiResult<Device[]>> {
  return call("GET", "/admin/sessions", { expected: 200, read: (json) => list(json, "sessions", readDevice) }, options);
}

export function revokeDevice(id: string, options: CallOptions): Promise<ApiResult<true>> {
  if (!isId(id)) {
    return Promise.resolve(NOT_AN_ID);
  }
  return call("DELETE", `/admin/sessions/${encodeURIComponent(id)}`, { expected: 204, read: nothing }, options);
}

function readBrowser(json: unknown): AdminBrowser | null {
  const body = record(json) ?? {};
  const { id, created_at: createdAt, expires_at: expiresAt, current } = body;
  const { id: userId, username, display_name: displayName } = record(body.user) ?? {};
  if (!text(id) || !number(createdAt) || !number(expiresAt) || typeof current !== "boolean") {
    return null;
  }
  if (!text(userId) || !text(username) || !text(displayName)) {
    return null;
  }
  return { id, user: { id: userId, username, displayName }, createdAt, expiresAt, current };
}

/** Browsers signed in to the admin, newest sign-in first (security review 1, minor 5). */
export function browsers(options: CallOptions): Promise<ApiResult<AdminBrowser[]>> {
  return call("GET", "/admin/browsers", { expected: 200, read: (json) => list(json, "browsers", readBrowser) }, options);
}

export function revokeBrowser(id: string, options: CallOptions): Promise<ApiResult<true>> {
  if (!isId(id)) {
    return Promise.resolve(NOT_AN_ID);
  }
  return call("DELETE", `/admin/browsers/${encodeURIComponent(id)}`, { expected: 204, read: nothing }, options);
}
