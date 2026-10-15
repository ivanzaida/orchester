import type { Invite, InviteState, Role } from "@/lib/model";
import { call, isId, NOT_AN_ID, type ApiResult, type CallOptions } from "@/lib/server/api";
import {
  list,
  number,
  numberOrNull,
  readRole,
  readUserRef,
  readUserRefOrNull,
  record,
  text,
  textOrNull,
  type Json,
} from "@/lib/server/team/read";

/** Invites to a project or to the server, by email through the cloud or with a code (contract, "Server API"). */

const KINDS = new Set<unknown>(["project", "server", "account"]);
const STATES = new Set<unknown>(["open", "accepted", "expired", "revoked"]);
const DELIVERIES = new Set<unknown>(["cloud", "code"]);

function readTimes(body: Json): Pick<Invite, "expiresAt" | "createdAt" | "endedAt"> | null {
  const { expires_at: expiresAt, created_at: createdAt } = body;
  const endedAt = numberOrNull(body.ended_at);
  return number(expiresAt) && number(createdAt) && endedAt !== undefined ? { expiresAt, createdAt, endedAt } : null;
}

function readPeople(body: Json): Pick<Invite, "user" | "by" | "endedBy"> | null {
  const user = readUserRefOrNull(body.user);
  const by = readUserRef(body.by);
  const endedBy = readUserRefOrNull(body.ended_by);
  return user !== undefined && by !== null && endedBy !== undefined ? { user, by, endedBy } : null;
}

function readInvite(json: unknown): Invite | null {
  const body = record(json) ?? {};
  const { id, kind, admin, delivery, state } = body;
  const email = textOrNull(body.email);
  const project = textOrNull(body.project);
  const role = body.role === null ? null : readRole(body.role);
  const times = readTimes(body);
  const people = readPeople(body);
  if (!text(id) || !KINDS.has(kind) || typeof admin !== "boolean" || !DELIVERIES.has(delivery) || !STATES.has(state)) {
    return null;
  }
  if (email === undefined || project === undefined || (body.role !== null && role === null) || times === null || people === null) {
    return null;
  }
  return {
    id,
    kind: kind as Invite["kind"],
    email,
    project,
    role,
    admin,
    delivery: delivery as Invite["delivery"],
    state: state as InviteState,
    ...times,
    ...people,
  };
}

/** Open invites and those that ended in the last 30 days; `project` limits them to one project. */
export function invites(project: string | null, options: CallOptions): Promise<ApiResult<Invite[]>> {
  if (project !== null && !isId(project)) {
    return Promise.resolve(NOT_AN_ID);
  }
  const query = new URLSearchParams({ ...(project === null ? {} : { project }), recent: "true" });
  return call("GET", `/invites?${query.toString()}`, { expected: 200, read: (json) => list(json, "invites", readInvite) }, options);
}

export type NewInvite =
  | { readonly kind: "project"; readonly delivery: "cloud" | "code"; readonly project: string; readonly role: Role; readonly email?: string }
  | { readonly kind: "server"; readonly delivery: "cloud" | "code"; readonly admin: boolean; readonly email?: string }
  | { readonly kind: "account"; readonly user: string };

/** What making an invite answers: the code to show once, or the signed invite to deliver to the cloud. */
export interface MadeInvite {
  readonly invite: Invite;
  readonly code: string | null;
  readonly signed: string | null;
}

function readMade(json: unknown): MadeInvite | null {
  const body = record(json) ?? {};
  const invite = readInvite(body.invite);
  const code = textOrNull(body.code);
  const signed = textOrNull(body.signed);
  return invite !== null && code !== undefined && signed !== undefined ? { invite, code, signed } : null;
}

function inviteBody(input: NewInvite): Json {
  switch (input.kind) {
    case "project":
      return { kind: "project", delivery: input.delivery, project: input.project, role: input.role, admin: false, ...(input.email === undefined ? {} : { email: input.email }) };
    case "server":
      return { kind: "server", delivery: input.delivery, admin: input.admin, ...(input.email === undefined ? {} : { email: input.email }) };
    case "account":
      return { kind: "account", delivery: "code", user: input.user, admin: false };
  }
}

export function createInvite(input: NewInvite, options: CallOptions): Promise<ApiResult<MadeInvite>> {
  const ids = [input.kind === "project" ? input.project : null, input.kind === "account" ? input.user : null];
  if (ids.some((id) => id !== null && !isId(id))) {
    return Promise.resolve(NOT_AN_ID);
  }
  return call("POST", "/invites", { body: inviteBody(input), expected: 201, read: readMade }, options);
}

/** `revocation` is what the admin delivers to the cloud for an email invite; `null` for a code. */
export function revokeInvite(id: string, options: CallOptions): Promise<ApiResult<{ revocation: string | null }>> {
  if (!isId(id)) {
    return Promise.resolve(NOT_AN_ID);
  }
  const read = (json: unknown) => {
    const revocation = textOrNull(record(json)?.revocation);
    return revocation === undefined ? null : { revocation };
  };
  return call("DELETE", `/invites/${encodeURIComponent(id)}`, { expected: 200, read }, options);
}
