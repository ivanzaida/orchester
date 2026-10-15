import type { Member, Project, Role } from "@/lib/model";
import { call, isId, NOT_AN_ID, nothing, type ApiResult, type CallOptions } from "@/lib/server/api";
import { field, list, number, readRole, readUserRef, record, text, textOrNull } from "@/lib/server/team/read";

/** Projects and their members (contract, "Server API"). */

function readOwners(json: unknown): Project["owners"] | null {
  if (!Array.isArray(json)) {
    return null;
  }
  const owners = json.map(readUserRef);
  return owners.every((owner) => owner !== null) ? owners : null;
}

function readProject(json: unknown): Project | null {
  const body = record(json) ?? {};
  const { id, slug, address, key, name, members, created_at: createdAt } = body;
  const remoteHint = textOrNull(body.remote_hint);
  const role = body.role === null ? null : readRole(body.role);
  const openInvites = body.open_invites;
  const owners = readOwners(body.owners);
  const createdBy = readUserRef(body.created_by);
  if (!text(id) || !text(slug) || !text(address) || !text(key) || !text(name) || remoteHint === undefined) {
    return null;
  }
  if ((body.role !== null && role === null) || !number(members) || !number(openInvites) || !number(createdAt)) {
    return null;
  }
  if (owners === null || createdBy === null) {
    return null;
  }
  return { id, slug, address, key, name, remoteHint, role, members, openInvites, owners, createdAt, createdBy };
}

/** Admins: every project; owners who are not admins: the projects they own. */
export function managedProjects(options: CallOptions): Promise<ApiResult<Project[]>> {
  return call("GET", "/admin/projects", { expected: 200, read: (json) => list(json, "projects", readProject) }, options);
}

/** The project's address once created; the rest of the answer is for the desktop's first upload. */
function readCreated(json: unknown): { address: string; slug: string } | null {
  const project = record(record(json)?.project);
  const { address, slug } = project ?? {};
  return text(address) && text(slug) ? { address, slug } : null;
}

export interface NewProject {
  readonly idempotencyKey: string;
  readonly name: string;
  readonly slug: string;
  readonly remoteHint: string | null;
}

export function createProject(input: NewProject, options: CallOptions): Promise<ApiResult<{ address: string; slug: string }>> {
  const hint = input.remoteHint === null ? {} : { remote_hint: input.remoteHint };
  const body = { idempotency_key: input.idempotencyKey, name: input.name, slug: input.slug, ...hint, tasks: [] };
  return call("POST", "/projects", { body, expected: 201, read: readCreated }, options);
}

/** `remoteHint: ""` clears the hint. */
export function updateProject(
  id: string,
  changes: { name?: string; remoteHint?: string },
  options: CallOptions,
): Promise<ApiResult<true>> {
  if (!isId(id)) {
    return Promise.resolve(NOT_AN_ID);
  }
  const body = {
    ...(changes.name === undefined ? {} : { name: changes.name }),
    ...(changes.remoteHint === undefined ? {} : { remote_hint: changes.remoteHint }),
  };
  return call("PATCH", `/projects/${encodeURIComponent(id)}`, { body, expected: 200, read: () => true }, options);
}

function readMember(json: unknown): Member | null {
  const body = record(json) ?? {};
  const user = readUserRef(body.user);
  const role = readRole(body.role);
  const { username, added_at: addedAt } = body;
  return user !== null && text(username) && role !== null && number(addedAt) ? { user, username, role, addedAt } : null;
}

export function members(projectId: string, options: CallOptions): Promise<ApiResult<Member[]>> {
  if (!isId(projectId)) {
    return Promise.resolve(NOT_AN_ID);
  }
  const path = `/projects/${encodeURIComponent(projectId)}/members`;
  return call("GET", path, { expected: 200, read: (json) => list(json, "members", readMember) }, options);
}

function memberPath(projectId: string, userId: string): string | null {
  return isId(projectId) && isId(userId)
    ? `/projects/${encodeURIComponent(projectId)}/members/${encodeURIComponent(userId)}`
    : null;
}

export function setRole(projectId: string, userId: string, role: Role, options: CallOptions): Promise<ApiResult<true>> {
  const path = memberPath(projectId, userId);
  if (path === null) {
    return Promise.resolve(NOT_AN_ID);
  }
  return call("PATCH", path, { body: { role }, expected: 200, read: (json) => field(json, "member", (m) => (m === undefined ? null : true)) }, options);
}

/** Removes a member, or the caller themself (*Leave*). */
export function removeMember(projectId: string, userId: string, options: CallOptions): Promise<ApiResult<true>> {
  const path = memberPath(projectId, userId);
  if (path === null) {
    return Promise.resolve(NOT_AN_ID);
  }
  return call("DELETE", path, { expected: 204, read: nothing }, options);
}
