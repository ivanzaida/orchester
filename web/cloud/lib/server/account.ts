import type { Device, Invite, InviteRole, Listing, Platform, Server } from "@/lib/account";
import { call, nothing, type ApiResult, type CallOptions } from "@/lib/server/api";
import { readAccount, record, type Account } from "@/lib/server/cloud";

/**
 * The signed-in account's endpoints (`docs/cloud-api.md`): `/devices`, `PATCH /me`, `DELETE /me`, `POST /auth/password`,
 * `GET /invites`, `POST /invites/{id}/decline` and `GET /servers`.
 */

const PLATFORMS = new Set<string>(["windows", "macos", "linux", "web"]);

/**
 * Ids come from a posted form, so they are checked before they go into an API path: the cloud's ids are UUIDs. Anything
 * else is refused here as the cloud would refuse a malformed request, and no request is made.
 */
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const NOT_AN_ID: ApiResult<never> = { ok: false, error: { kind: "refused", status: 400, code: "bad_request" } };

function text(value: unknown): value is string {
  return typeof value === "string";
}

/** The items of `json[field]` that `keep` keeps, each read; `null` when the list or any kept item is not as expected. */
function list<T>(
  json: unknown,
  field: string,
  read: (item: unknown) => T | null,
  keep: (item: unknown) => boolean = () => true,
): T[] | null {
  const items = record(json)?.[field];
  if (!Array.isArray(items)) {
    return null;
  }
  const parsed = items.filter((item) => keep(item)).map(read);
  return parsed.every((item) => item !== null) ? (parsed) : null;
}

function readDevice(json: unknown): Device | null {
  const { id, name, platform, last_used_at: lastUsedAt, current } = record(json) ?? {};
  if (!text(id) || !text(name) || !text(platform) || !PLATFORMS.has(platform)) {
    return null;
  }
  if (typeof lastUsedAt !== "number" || typeof current !== "boolean") {
    return null;
  }
  return { id, name, platform: platform as Platform, lastUsedAt, current };
}

function readProject(json: unknown): Invite["project"] | undefined {
  if (json === null) {
    return null;
  }
  const { name, address } = record(json) ?? {};
  return text(name) && text(address) ? { name, address } : undefined;
}

/** A project invite's role (`owner` or `member`); a server invite carries none. `undefined` when it is not one. */
function readRole(kind: "project" | "server", role: unknown): InviteRole | null | undefined {
  if (kind === "server") {
    return role === null ? null : undefined;
  }
  return role === "owner" || role === "member" ? role : undefined;
}

function readInvite(json: unknown): Invite | null {
  const { id, kind, role: wireRole, inviter_display_name: inviterName, expires_at: expiresAt, server, project } =
    record(json) ?? {};
  // Absent from a cloud before two-factor authentication, which asked for none.
  const needsTwoFactor = record(json)?.needs_two_factor === true;
  const { name: serverName, address: serverAddress } = record(server) ?? {};
  const invitedTo = readProject(project);
  if (!text(id) || (kind !== "project" && kind !== "server") || !text(inviterName)) {
    return null;
  }
  const role = readRole(kind, wireRole);
  if (role === undefined) {
    return null;
  }
  if (!text(serverName) || !text(serverAddress) || invitedTo === undefined || typeof expiresAt !== "number") {
    return null;
  }
  return { id, kind, role, serverName, serverAddress, project: invitedTo, inviterName, expiresAt, needsTwoFactor };
}

function readServer(json: unknown): Server | null {
  const { id, name, address, owner, joined_at: since } = record(json) ?? {};
  if (!text(id) || !text(name) || !text(address) || typeof owner !== "boolean" || typeof since !== "number") {
    return null;
  }
  return { id, name, address, owner, since };
}

export function devices(options: CallOptions): Promise<ApiResult<Device[]>> {
  return call("GET", "/devices", { expected: 200, read: (json) => list(json, "devices", readDevice) }, options);
}

export function revokeDevice(id: string, options: CallOptions): Promise<ApiResult<true>> {
  if (!UUID.test(id)) {
    return Promise.resolve(NOT_AN_ID);
  }
  return call("DELETE", `/devices/${encodeURIComponent(id)}`, { expected: 204, read: nothing }, options);
}

export function rename(displayName: string, options: CallOptions): Promise<ApiResult<Account>> {
  const read = (json: unknown) => readAccount(record(json)?.account);
  return call("PATCH", "/me", { body: { display_name: displayName }, expected: 200, read }, options);
}

export function changePassword(current: string, next: string, options: CallOptions): Promise<ApiResult<true>> {
  const body = { current_password: current, new_password: next };
  return call("POST", "/auth/password", { body, expected: 204, read: nothing }, options);
}

async function listing<T>(path: string, read: (json: unknown) => T[] | null, options: CallOptions): Promise<Listing<T>> {
  const result = await call("GET", path, { expected: 200, read }, options);
  return result.ok ? { kind: "list", items: result.value } : { kind: "failed" };
}

/**
 * `GET /invites` → `{ invites: [Invite] }`, the open invites to this account's email. An invite of kind `account`
 * (a team server linking an account it already has to this cloud identity) has no row in the Cloud frames, so the
 * page does not list it; it is accepted in the Orchester app.
 */
export function invites(options: CallOptions): Promise<Listing<Invite>> {
  const drawn = (item: unknown) => record(item)?.kind !== "account";
  return listing("/invites", (json) => list(json, "invites", readInvite, drawn), options);
}

/** `GET /servers` → `{ servers: [Server] }`, the team servers the account owns or joined. */
export function servers(options: CallOptions): Promise<Listing<Server>> {
  return listing("/servers", (json) => list(json, "servers", readServer), options);
}

/** `DELETE /me { password }` → `204`: the account and everything the cloud holds about it. */
export function deleteAccount(password: string, options: CallOptions): Promise<ApiResult<true>> {
  return call("DELETE", "/me", { body: { password }, expected: 204, read: nothing }, options);
}

/** `POST /invites/{id}/decline` → `204`. The team server keeps its copy until it expires; nothing joins with it. */
export function declineInvite(id: string, options: CallOptions): Promise<ApiResult<true>> {
  if (!UUID.test(id)) {
    return Promise.resolve(NOT_AN_ID);
  }
  return call("POST", `/invites/${encodeURIComponent(id)}/decline`, { expected: 204, read: nothing }, options);
}
