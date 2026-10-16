/** The fake team server's admin side: projects, members, invites, people, devices, settings and the cloud link. */
import { DAY, SERVER_ID, id, type FakeInvite, type FakeState } from "./fixtures.ts";
import { cloudJson, inviteJson, managedProject, ok, refuse, settingsJson, userJson, userRef, type Answer } from "./shapes.ts";
import type { Request, World } from "./team-auth.ts";

function admin(state: FakeState, me: string): boolean {
  return state.users.find((u) => u.id === me)?.admin === true;
}

function owns(state: FakeState, me: string, projectId: string): boolean {
  return state.projects.find((p) => p.id === projectId)?.members.some((m) => m.user === me && m.role === "owner") === true;
}

function managed(state: FakeState, me: string) {
  return admin(state, me) ? state.projects : state.projects.filter((p) => owns(state, me, p.id));
}

function createProject(world: World, request: Request, me: string): Answer {
  const { name, slug, remote_hint: hint } = request.body;
  if (typeof name !== "string" || typeof slug !== "string" || !/^[a-z0-9]+(-[a-z0-9]+)*$/.test(slug)) {
    return refuse(422, "invalid");
  }
  if (["admin", "api", "auth"].includes(slug) || world.state.projects.some((p) => p.slug === slug)) {
    return refuse(409, "slug_taken");
  }
  const now = Date.now();
  const project = { id: id(500 + world.state.projects.length), slug, key: slug.slice(0, 3).toUpperCase(), name, remote_hint: typeof hint === "string" ? hint : null, created_at: now, created_by: me, members: [{ user: me, role: "owner" as const, added_at: now }] };
  world.state.projects.push(project);
  const shared = managedProject(world.state, project, me, request.place);
  return ok({ project: { ...shared, next_number: 1 }, tasks: [], cursor: 0, dropped_links: [] }, 201);
}

function updateProject(world: World, request: Request, projectId: string): Answer {
  const project = world.state.projects.find((p) => p.id === projectId);
  if (project === undefined) {
    return refuse(404, "not_found");
  }
  const { name, remote_hint: hint } = request.body;
  if (typeof name === "string") {
    project.name = name;
  }
  if (typeof hint === "string") {
    project.remote_hint = hint === "" ? null : hint;
  }
  return ok({ project: managedProject(world.state, project, request.me ?? "", request.place) });
}

function members(world: World, projectId: string): Answer {
  const project = world.state.projects.find((p) => p.id === projectId);
  if (project === undefined) {
    return refuse(404, "not_found");
  }
  const list = project.members.map((m) => ({ user: userRef(world.state, m.user), username: world.state.users.find((u) => u.id === m.user)?.username ?? "", role: m.role, added_at: m.added_at }));
  return ok({ members: list });
}

function changeMember(world: World, request: Request, projectId: string, userId: string): Answer {
  const project = world.state.projects.find((p) => p.id === projectId);
  const member = project?.members.find((m) => m.user === userId);
  if (project === undefined || member === undefined) {
    return refuse(404, "not_found");
  }
  const owners = project.members.filter((m) => m.role === "owner");
  const losesOwner = member.role === "owner" && owners.length === 1 && (request.method === "DELETE" || request.body.role === "member");
  if (losesOwner) {
    return refuse(409, "last_owner");
  }
  if (request.method === "DELETE") {
    project.members = project.members.filter((m) => m !== member);
    return { status: 204 };
  }
  member.role = request.body.role === "owner" ? "owner" : "member";
  return ok({ member: { user: userRef(world.state, userId), role: member.role } });
}

function invites(world: World, request: Request, query: URLSearchParams): Answer {
  const project = query.get("project");
  const list = world.state.invites.filter((i) => (project === null ? true : i.project === project));
  return ok({ invites: list.map((i) => inviteJson(world.state, i)) });
}

function newInvite(world: World, request: Request, me: string): Answer {
  const { kind, delivery, email, project, role, user } = request.body;
  if (typeof email === "string" && email.startsWith("limit")) {
    return refuse(429, "rate_limited", 3 * 3600);
  }
  // The server makes invites only for a sign-in with two-factor authentication (`mfa_required`).
  if (world.state.no_mfa === true || (typeof email === "string" && email.startsWith("no-mfa"))) {
    return refuse(403, "mfa_required");
  }
  if (kind === "account" && world.state.users.find((u) => u.id === user)?.admin === true) {
    return refuse(403, "forbidden");
  }
  if (delivery === "cloud" && world.state.cloud === null) {
    return refuse(403, "cloud_off");
  }
  const now = Date.now();
  const code = delivery === "code";
  const invite: FakeInvite = {
    id: id(600 + world.state.invites.length), kind: kind === "server" || kind === "account" ? kind : "project",
    email: typeof email === "string" ? email : null, project: typeof project === "string" ? project : null,
    role: role === "owner" ? "owner" : kind === "project" ? "member" : null, user: typeof user === "string" ? user : null,
    admin: request.body.admin === true, by: me, delivery: code ? "code" : "cloud",
    expires_at: now + (kind === "account" ? DAY : code ? 3 * DAY : 7 * DAY), created_at: now, state: "open", ended_at: null, ended_by: null,
  };
  world.state.invites.push(invite);
  const signed = code ? null : `fake-invite.${Buffer.from(invite.email ?? "").toString("base64url")}`;
  return ok({ invite: inviteJson(world.state, invite), code: code ? "K7QM-3XRD" : null, signed }, 201);
}

function revoke(world: World, inviteId: string, me: string): Answer {
  const invite = world.state.invites.find((i) => i.id === inviteId);
  if (invite === undefined || invite.state !== "open") {
    return refuse(404, "not_found");
  }
  const email = invite.email;
  Object.assign(invite, { state: "revoked", ended_at: Date.now(), ended_by: me });
  return ok({ revocation: invite.delivery === "cloud" ? `fake-revocation.${Buffer.from(email ?? "").toString("base64url")}` : null });
}

function users(world: World): Answer {
  const list = world.state.users.map((u) => ({ ...userJson(u), projects: world.state.projects.filter((p) => p.members.some((m) => m.user === u.id)).length }));
  return ok({ users: list });
}

function updateUser(world: World, request: Request, userId: string): Answer {
  const user = world.state.users.find((u) => u.id === userId);
  if (user === undefined) {
    return refuse(404, "no_such_user");
  }
  const { admin: makeAdmin, disabled } = request.body;
  const admins = world.state.users.filter((u) => u.admin && !u.disabled);
  if (user.admin && admins.length === 1 && (makeAdmin === false || disabled === true)) {
    return refuse(409, "last_admin");
  }
  if (typeof makeAdmin === "boolean") {
    user.admin = makeAdmin;
  }
  if (typeof disabled === "boolean") {
    user.disabled = disabled;
  }
  return ok({ user: userJson(user) });
}

function sessions(world: World): Answer {
  const list = [...world.state.devices]
    .sort((a, b) => b.last_used_at - a.last_used_at)
    .map((d) => {
      const user = world.state.users.find((u) => u.id === d.user);
      return { ...d, user: { id: d.user, username: user?.username ?? "", display_name: user?.display_name ?? "" }, current: false };
    });
  return ok({ sessions: list });
}

/** The fixture's browsers and the asking one, newest sign-in first; the fake has one session per sign-in. */
function browsers(world: World, me: string): Answer {
  const asking = { id: id(50), user: me, created_at: Date.now() - 5 * 60 * 1000, current: true };
  const list = [...world.state.browsers.map((b) => ({ ...b, current: false })), asking]
    .sort((a, b) => b.created_at - a.created_at)
    .map((b) => {
      const user = world.state.users.find((u) => u.id === b.user);
      return { id: b.id, user: { id: b.user, username: user?.username ?? "", display_name: user?.display_name ?? "" }, via: "cloud",
        created_at: b.created_at, last_used_at: b.created_at, expires_at: b.created_at + 12 * 60 * 60 * 1000, current: b.current };
    });
  return ok({ browsers: list });
}

function changeCloud(world: World, body: Record<string, unknown>, me: string): Answer {
  const now = Date.now();
  if (body.link === "start") {
    return ok({ cloud: cloudJson(world.state), statement: "fake-statement" });
  }
  if (typeof body.link === "object" && body.link !== null) {
    world.state.cloud = { server: SERVER_ID, root_fingerprint: "nQ3hW8vK2pXcT5yLmR7aZ1bF9dG4sJ6e", accepting: true, key_set_issued_at: now, key_set_expires_at: now + 30 * DAY, linked_at: now, linked_by: me };
    return ok({ cloud: cloudJson(world.state), statement: null });
  }
  const cloud = world.state.cloud;
  if (cloud === null) {
    return refuse(409, "conflict");
  }
  if (typeof body.accepting === "boolean") {
    cloud.accepting = body.accepting;
  } else if (typeof body.pin === "string") {
    Object.assign(cloud, { root_fingerprint: body.pin, key_set_issued_at: null, key_set_expires_at: null });
  } else if (body.unlink === true) {
    world.state.cloud = null;
  }
  return ok({ cloud: cloudJson(world.state), statement: null });
}

function updateSettings(world: World, body: Record<string, unknown>): Answer {
  if (typeof body.join_approval === "boolean") {
    world.state.join_approval = body.join_approval;
  }
  if (typeof body.name === "string") {
    world.state.name = body.name;
  }
  return ok(settingsJson(world.state));
}

type Route = [method: string, pattern: RegExp, answer: (world: World, request: Request, me: string, match: string[], query: URLSearchParams) => Answer, adminOnly: boolean];

const ROUTES: Route[] = [
  ["GET", /^\/api\/v1\/admin\/projects$/, (w, r, me) => ok({ projects: managed(w.state, me).map((p) => managedProject(w.state, p, me, r.place)) }), false],
  ["POST", /^\/api\/v1\/projects$/, (w, r, me) => createProject(w, r, me), true],
  ["PATCH", /^\/api\/v1\/projects\/([^/]+)$/, (w, r, _, m) => updateProject(w, r, m[1] ?? ""), false],
  ["GET", /^\/api\/v1\/projects\/([^/]+)\/members$/, (w, _r, _m, m) => members(w, m[1] ?? ""), false],
  ["PATCH", /^\/api\/v1\/projects\/([^/]+)\/members\/([^/]+)$/, (w, r, _, m) => changeMember(w, r, m[1] ?? "", m[2] ?? ""), false],
  ["DELETE", /^\/api\/v1\/projects\/([^/]+)\/members\/([^/]+)$/, (w, r, _, m) => changeMember(w, r, m[1] ?? "", m[2] ?? ""), false],
  ["GET", /^\/api\/v1\/invites$/, (w, r, _me, _m, q) => invites(w, r, q), false],
  ["POST", /^\/api\/v1\/invites$/, (w, r, me) => newInvite(w, r, me), false],
  ["DELETE", /^\/api\/v1\/invites\/([^/]+)$/, (w, _r, me, m) => revoke(w, m[1] ?? "", me), false],
  ["GET", /^\/api\/v1\/admin\/users$/, (w) => users(w), true],
  ["PATCH", /^\/api\/v1\/admin\/users\/([^/]+)$/, (w, r, _, m) => updateUser(w, r, m[1] ?? ""), true],
  ["GET", /^\/api\/v1\/admin\/sessions$/, (w) => sessions(w), true],
  ["DELETE", /^\/api\/v1\/admin\/sessions\/([^/]+)$/, (w, _r, _me, m) => {
    w.state.devices = w.state.devices.filter((d) => d.id !== m[1]);
    return { status: 204 };
  }, true],
  ["GET", /^\/api\/v1\/admin\/browsers$/, (w, _r, me) => browsers(w, me), true],
  ["DELETE", /^\/api\/v1\/admin\/browsers\/([^/]+)$/, (w, _r, _me, m) => {
    const before = w.state.browsers.length;
    w.state.browsers = w.state.browsers.filter((b) => b.id !== m[1]);
    return w.state.browsers.length === before ? refuse(404, "not_found") : { status: 204 };
  }, true],
  ["GET", /^\/api\/v1\/server\/settings$/, (w) => ok(settingsJson(w.state)), true],
  ["PATCH", /^\/api\/v1\/server\/settings$/, (w, r) => updateSettings(w, r.body), true],
  ["GET", /^\/api\/v1\/server\/cloud$/, (w) => ok({ cloud: cloudJson(w.state), statement: null }), true],
  ["PATCH", /^\/api\/v1\/server\/cloud$/, (w, r, me) => changeCloud(w, r.body, me), true],
];

/** The signed-in admin routes; `undefined`: not one of these. */
export function adminRoute(world: World, request: Request, query: URLSearchParams): Answer | undefined {
  for (const [method, pattern, answer, adminOnly] of ROUTES) {
    const match = pattern.exec(request.path);
    if (method !== request.method || match === null) {
      continue;
    }
    if (request.me === null) {
      return refuse(401, "unauthorized");
    }
    if (adminOnly && !admin(world.state, request.me)) {
      return refuse(403, "forbidden");
    }
    return answer(world, request, request.me, [...match], query);
  }
  return undefined;
}
