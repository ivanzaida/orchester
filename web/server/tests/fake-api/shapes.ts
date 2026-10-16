/** The fake team server's answers in the contract's JSON shapes (`.tmp/sa-run/contract.md`, "Server API"). */
import type { FakeInvite, FakeProject, FakeState, FakeUser } from "./fixtures.ts";

export interface Answer {
  status: number;
  body?: unknown;
  retryAfter?: number;
}

export const refuse = (status: number, error: string, retryAfter?: number): Answer => ({
  status,
  body: { error, message: "fake" },
  ...(retryAfter === undefined ? {} : { retryAfter }),
});

export const ok = (body: unknown, status = 200): Answer => ({ status, body });

export function userRef(state: FakeState, id: string | null) {
  const user = state.users.find((u) => u.id === id);
  return user === undefined ? null : { id: user.id, display_name: user.display_name };
}

export function userJson(user: FakeUser) {
  return { id: user.id, username: user.username, display_name: user.display_name, admin: user.admin, disabled: user.disabled, cloud: user.cloud };
}

export function address(host: string, proto: string, slug: string): string {
  return `${proto}://${host}/${slug}`;
}

export function managedProject(state: FakeState, project: FakeProject, me: string, place: { host: string; proto: string }) {
  const role = project.members.find((m) => m.user === me)?.role ?? null;
  return {
    id: project.id,
    slug: project.slug,
    address: address(place.host, place.proto, project.slug),
    key: project.key,
    name: project.name,
    remote_hint: project.remote_hint,
    role,
    members: project.members.length,
    open_invites: state.invites.filter((i) => i.project === project.id && i.state === "open").length,
    owners: project.members.filter((m) => m.role === "owner").map((m) => userRef(state, m.user)),
    created_at: project.created_at,
    created_by: userRef(state, project.created_by),
  };
}

export function inviteJson(state: FakeState, invite: FakeInvite) {
  return {
    id: invite.id,
    kind: invite.kind,
    email: invite.state === "open" ? invite.email : null,
    project: invite.project,
    role: invite.role,
    user: userRef(state, invite.user),
    admin: invite.admin,
    by: userRef(state, invite.by),
    delivery: invite.delivery,
    expires_at: invite.expires_at,
    created_at: invite.created_at,
    state: invite.state,
    ended_at: invite.ended_at,
    ended_by: userRef(state, invite.ended_by),
  };
}

export function serverJson(state: FakeState) {
  const cloud = state.cloud === null ? null : { server: state.cloud.server, root_fingerprint: state.cloud.root_fingerprint, accepting: state.cloud.accepting };
  return { name: state.name, api_version: 3, needs_setup: state.needs_setup, cloud, certificate: state.certificate, version: state.version };
}

export function cloudJson(state: FakeState) {
  const cloud = state.cloud;
  if (cloud === null) {
    return { server: "0199f0a0-0000-7000-8000-000000000099", root_fingerprint: null, accepting: false, key_set_issued_at: null, key_set_expires_at: null, linked_at: null, linked_by: null };
  }
  return { ...cloud, linked_by: userRef(state, cloud.linked_by) };
}

export function settingsJson(state: FakeState) {
  return { settings: { join_approval: state.join_approval, name: state.name }, check_code: state.certificate?.check_code ?? null, certificate: state.certificate };
}
