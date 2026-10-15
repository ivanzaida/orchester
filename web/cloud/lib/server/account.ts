import type { Device, Invite, Listing, Platform, Server } from "@/lib/account";
import { call, nothing, type ApiResult, type CallOptions } from "@/lib/server/api";
import { readAccount, record, type Account } from "@/lib/server/cloud";

/**
 * The signed-in account's endpoints (`docs/cloud-api.md`): `/devices`, `PATCH /me`, `POST /auth/password`. Invites and
 * the server registry are not in the cloud yet; their shapes here are assumed from `server.md` ("Orchester Cloud") and
 * listed for the backend in `.tmp/cloud-web-run/report.md`: `GET /invites`, `GET /servers`,
 * `POST /invites/{id}/decline`.
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

function list<T>(json: unknown, field: string, read: (item: unknown) => T | null): T[] | null {
  const items = record(json)?.[field];
  if (!Array.isArray(items)) {
    return null;
  }
  const parsed = items.map(read);
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

function readInvite(json: unknown): Invite | null {
  const { id, kind, role, inviter_display_name: inviterName, expires_at: expiresAt, server, project } = record(json) ?? {};
  const { name: serverName, address: serverAddress } = record(server) ?? {};
  const invitedTo = readProject(project);
  if (!text(id) || (kind !== "project" && kind !== "server") || !text(role) || !text(inviterName)) {
    return null;
  }
  if (!text(serverName) || !text(serverAddress) || invitedTo === undefined || typeof expiresAt !== "number") {
    return null;
  }
  return { id, kind, role, serverName, serverAddress, project: invitedTo, inviterName, expiresAt };
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

async function listing<T>(path: string, field: string, read: (item: unknown) => T | null, options: CallOptions) {
  const result = await call("GET", path, { expected: 200, read: (json) => list(json, field, read) }, options);
  if (result.ok) {
    return { kind: "list", items: result.value } satisfies Listing<T>;
  }
  const missing = result.error.kind === "refused" && result.error.code === "not_found";
  return (missing ? { kind: "notYet" } : { kind: "failed" }) satisfies Listing<T>;
}

/** Assumed: `GET /invites` → `{ invites: [Invite] }`, the open invites to this account's email. */
export function invites(options: CallOptions): Promise<Listing<Invite>> {
  return listing("/invites", "invites", readInvite, options);
}

/** Assumed: `GET /servers` → `{ servers: [Server] }`, the team servers the account owns or joined. */
export function servers(options: CallOptions): Promise<Listing<Server>> {
  return listing("/servers", "servers", readServer, options);
}

/** Assumed: `POST /invites/{id}/decline` → `204`; the cloud tells the team server. */
export function declineInvite(id: string, options: CallOptions): Promise<ApiResult<true>> {
  if (!UUID.test(id)) {
    return Promise.resolve(NOT_AN_ID);
  }
  return call("POST", `/invites/${encodeURIComponent(id)}/decline`, { expected: 204, read: nothing }, options);
}
