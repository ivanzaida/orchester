/**
 * The fake team server's sign-in side: setup, codes, cloud sign-ins, the session, join approvals. What answers what
 * is listed in `web/server/README.md` ("The fake API's states").
 */
import { randomBytes } from "node:crypto";

import { IVAN, MARIA, OLEH, SETUP_CODE, id, type FakeState } from "./fixtures.ts";
import { ok, refuse, serverJson, userJson, userRef, type Answer } from "./shapes.ts";

export interface Request {
  readonly method: string;
  readonly path: string;
  readonly body: Record<string, unknown>;
  readonly me: string | null;
  readonly token: string | null;
  readonly place: { host: string; proto: string };
}

export interface World {
  state: FakeState;
  readonly tokens: Map<string, string>;
}

/** Codes the page tests type: who each signs in, or how it is refused. */
const CODES: Record<string, string> = { "ADMN-0001": IVAN, "OWNR-0001": MARIA };
/** Cloud identities by email, as the fake cloud's `/authorize` hands them out. */
const IDENTITIES: Record<string, string> = { "ivan@example.com": IVAN, "maria@example.com": MARIA, "dana@example.com": OLEH };

function normal(code: unknown): string {
  return typeof code === "string" ? code.toUpperCase().replace(/[\s-]/g, "") : "";
}

function same(a: unknown, b: string): boolean {
  return normal(a) === normal(b);
}

/** An admin, or someone who owns a project: anyone else has nothing to manage here. */
function manages(world: World, user: string): boolean {
  const account = world.state.users.find((u) => u.id === user);
  if (account?.admin === true) {
    return true;
  }
  return world.state.projects.some((p) => p.members.some((m) => m.user === user && m.role === "owner"));
}

function issue(world: World, user: string): Answer {
  const account = world.state.users.find((u) => u.id === user);
  if (account === undefined) {
    return refuse(500, "internal");
  }
  if (!manages(world, user)) {
    return refuse(403, "nothing_to_manage");
  }
  const token = `adm_fake_${randomBytes(12).toString("hex")}`;
  world.tokens.set(token, user);
  const session = { id: id(900 + world.tokens.size), device: "browser", created_at: Date.now(), last_used_at: Date.now(), current: true, via: "code" };
  return ok({ token, session, user: userJson(account), joined: null }, 201);
}

function setup(world: World, body: Record<string, unknown>): Answer {
  if (normal(body.code).startsWith("LIMT")) {
    return refuse(429, "rate_limited", 900);
  }
  if (!world.state.needs_setup || !same(body.code, SETUP_CODE)) {
    return refuse(401, "invalid_code");
  }
  return { status: 204 };
}

function firstAdmin(world: World, body: Record<string, unknown>): Answer {
  const { username, display_name: displayName } = body;
  if (typeof username !== "string" || typeof displayName !== "string" || username === "") {
    return refuse(422, "invalid");
  }
  if (username === "taken") {
    return refuse(409, "username_taken");
  }
  world.state.users = [{ id: IVAN, username, display_name: displayName, admin: true, disabled: false, cloud: false, email: null }];
  world.state.needs_setup = false;
  return issue(world, IVAN);
}

function code(world: World, body: Record<string, unknown>): Answer {
  if (normal(body.code).startsWith("LIMT")) {
    return refuse(429, "rate_limited", 900);
  }
  if (world.state.needs_setup && same(body.code, SETUP_CODE)) {
    return firstAdmin(world, body);
  }
  if (same(body.code, "NONE-0001")) {
    return refuse(403, "nothing_to_manage");
  }
  const user = Object.entries(CODES).find(([known]) => same(body.code, known))?.[1];
  return user === undefined ? refuse(401, "invalid_code") : issue(world, user);
}

/** The email inside a fake identity token (`header.payload.signature`, the payload base64url JSON). */
export function tokenEmail(token: unknown): string | null {
  const payload = typeof token === "string" ? token.split(".")[1] : undefined;
  if (payload === undefined) {
    return null;
  }
  const claims = JSON.parse(Buffer.from(payload, "base64url").toString()) as { email?: unknown };
  return typeof claims.email === "string" ? claims.email : null;
}

function cloudSignIn(world: World, body: Record<string, unknown>): Answer {
  const cloud = world.state.cloud;
  if (cloud === null || !cloud.accepting) {
    return refuse(403, "cloud_off");
  }
  const email = tokenEmail(body.identity_token);
  if (email === "approval@example.com") {
    return refuse(403, "approval_required");
  }
  if (email === "stale@example.com") {
    return refuse(409, "key_set_stale");
  }
  if (email === null) {
    return refuse(401, "invalid_identity");
  }
  const user = IDENTITIES[email];
  return user === undefined ? refuse(403, "not_invited") : issue(world, user);
}

function approvals(world: World): Answer {
  const list = world.state.approvals.map((a) => ({ ...a, user: userRef(world.state, a.user), state: "waiting" }));
  return ok({ approvals: list });
}

function decide(world: World, approvalId: string, body: Record<string, unknown>): Answer {
  const approval = world.state.approvals.find((a) => a.id === approvalId);
  if (approval === undefined) {
    return refuse(404, "not_found");
  }
  world.state.approvals = world.state.approvals.filter((a) => a.id !== approvalId);
  if (body.approve === true) {
    const now = Date.now();
    world.state.devices.unshift({ id: id(950), device: approval.device, user: approval.user, via: "cloud", created_at: now, last_used_at: now, expires_at: now + 90 * 86_400_000 });
  }
  return ok({ approval: { ...approval, user: userRef(world.state, approval.user), state: body.approve === true ? "approved" : "refused" } });
}

/** The routes without a session, and the session's own. `undefined`: not one of these. */
export function authRoute(world: World, request: Request): Answer | undefined {
  const { method, path, body, me } = request;
  const signed = (answer: () => Answer) => (me === null ? refuse(401, "unauthorized") : answer());
  switch (`${method} ${path}`) {
    case "GET /api/v1/server":
      return ok(serverJson(world.state));
    case "POST /api/v1/auth/setup":
      return setup(world, body);
    case "POST /api/v1/auth/code":
      return code(world, body);
    case "POST /api/v1/auth/cloud":
      return cloudSignIn(world, body);
    case "GET /api/v1/auth/me":
      return signed(() => {
        const user = world.state.users.find((u) => u.id === me);
        return user === undefined ? refuse(401, "unauthorized") : ok({ user: userJson(user) });
      });
    case "DELETE /api/v1/auth/sessions/current":
      return signed(() => {
        world.tokens.delete(request.token ?? "");
        return { status: 204 };
      });
    case "GET /api/v1/auth/approvals":
      return signed(() => approvals(world));
    default:
      break;
  }
  const decision = /^\/api\/v1\/auth\/approvals\/([^/]+)$/.exec(path);
  if (method === "POST" && decision !== null) {
    return signed(() => decide(world, decision[1] ?? "", body));
  }
  return undefined;
}
