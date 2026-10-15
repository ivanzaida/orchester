/**
 * The fake cloud's signed-in side: accounts with devices, invites and servers, and the sessions that read them.
 * Three accounts (`web/cloud/README.md`): Mira with everything (`H47EAc`), a new account (`Z4Q3F`), and one on a
 * cloud like C1a's, which has no invites or server registry yet and answers `404` for them.
 */

export const RIGHT_PASSWORD = "correct horse battery";
export const FULL = "mira.koval@example.com";
export const NEW = "new.account@example.com";
export const EARLY = "early.cloud@example.com";

export interface Answer {
  status: number;
  body?: unknown;
  retryAfter?: number;
}

// The cloud's ids are UUIDs (the site refuses anything else in an API path).
const DESKTOP = "0199f0a0-0000-7000-8000-000000000001";
export const MBP = "0199f0a0-0000-7000-8000-000000000002";
const INVITE_PROJECT = "0199f0a0-0000-7000-8000-000000000011";
const INVITE_SERVER = "0199f0a0-0000-7000-8000-000000000012";

const HOUR = 60 * 60 * 1000;
const DAY = 24 * HOUR;

interface FakeDevice {
  id: string;
  name: string;
  platform: string;
  last_used_at: number;
}

interface FakeAccount {
  id: string;
  email: string;
  display_name: string;
  password: string;
  devices: FakeDevice[];
  invites: unknown[] | null;
  servers: unknown[] | null;
}

function fullAccount(now: number): FakeAccount {
  return {
    id: "account-full",
    email: FULL,
    display_name: "Mira Koval",
    password: RIGHT_PASSWORD,
    devices: [
      { id: DESKTOP, name: "mira-desktop", platform: "windows", last_used_at: now - 2 * HOUR },
      { id: MBP, name: "mira-mbp", platform: "macos", last_used_at: now - 3 * DAY },
    ],
    invites: [
      {
        id: INVITE_PROJECT,
        kind: "project",
        role: "Member",
        server: { id: "server-northwind", name: "Northwind studio", address: "orchester.northwind.example" },
        project: { name: "mobile-app", address: "orchester.northwind.example/mobile-app" },
        inviter_display_name: "Tomas Berg",
        expires_at: now + 5 * DAY,
      },
      {
        id: INVITE_SERVER,
        kind: "server",
        role: "Admin",
        server: { id: "server-atlas", name: "Atlas research", address: "orchester.atlas.example" },
        project: null,
        inviter_display_name: "Lena Ortiz",
        expires_at: now + 7 * DAY - HOUR,
      },
    ],
    servers: [
      { id: "server-guild", name: "Platform guild", address: "orchester.guild.example", owner: false, joined_at: now - 13 * DAY },
      { id: "server-lab", name: "Koval lab", address: "lab.koval.example", owner: true, joined_at: now - 17 * DAY },
    ],
  };
}

function plainAccount(id: string, email: string, name: string, registry: unknown[] | null): FakeAccount {
  return { id, email, display_name: name, password: RIGHT_PASSWORD, devices: [], invites: registry, servers: registry };
}

const accounts = new Map<string, FakeAccount>();
const sessions = new Map<string, { email: string; device: string }>();
let issued = 0;

/** Forgets every change and session, for a test that needs the accounts as drawn. */
export function resetAccounts(): void {
  accounts.clear();
  sessions.clear();
}

function account(email: string): FakeAccount | undefined {
  if (!accounts.has(email)) {
    const now = Date.now();
    const made: Record<string, () => FakeAccount> = {
      [FULL]: () => fullAccount(now),
      [NEW]: () => plainAccount("account-new", NEW, "Nova Lind", []),
      [EARLY]: () => plainAccount("account-early", EARLY, "Erik Hale", null),
    };
    const make = made[email];
    if (make !== undefined) {
      accounts.set(email, make());
    }
  }
  return accounts.get(email);
}

const refuse = (status: number, error: string): Answer => ({ status, body: { error, message: "fake" } });

function wire(a: FakeAccount) {
  return { id: a.id, email: a.email, display_name: a.display_name, created_at: 0 };
}

export function signInAccount(email: unknown, password: unknown, device: unknown): Answer {
  const found = typeof email === "string" ? account(email) : undefined;
  if (found === undefined || password !== found.password) {
    return refuse(401, "invalid_credentials");
  }
  issued += 1;
  const token = `ocd_fake_web_token_${issued}`;
  const name = (device as { name?: unknown } | null)?.name;
  const signedIn = { id: `0199f0a0-0000-7000-9000-${String(issued).padStart(12, "0")}`, name: typeof name === "string" ? name : "Web browser", platform: "web", last_used_at: Date.now() };
  // One browser per account, so the page shows the designed three devices however often the tests sign in.
  for (const old of found.devices.filter((d) => d.platform === "web")) {
    revoke(found, old.id);
  }
  found.devices.unshift(signedIn);
  sessions.set(token, { email: found.email, device: signedIn.id });
  const current = { ...signedIn, created_at: 0, current: true };
  return { status: 201, body: { token, device: current, account: wire(found) } };
}

function devices(a: FakeAccount, current: string): Answer {
  const list = a.devices.map((d) => ({ ...d, created_at: 0, current: d.id === current }));
  return { status: 200, body: { devices: list } };
}

function revoke(a: FakeAccount, id: string): Answer {
  const before = a.devices.length;
  a.devices = a.devices.filter((d) => d.id !== id);
  for (const [token, session] of sessions) {
    if (session.device === id) {
      sessions.delete(token);
    }
  }
  return a.devices.length < before ? { status: 204 } : refuse(404, "not_found");
}

function changePassword(a: FakeAccount, body: Record<string, unknown>, current: string): Answer {
  if (body.current_password !== a.password) {
    return refuse(401, "invalid_credentials");
  }
  a.devices = a.devices.filter((d) => d.id === current);
  return { status: 204 };
}

function rename(a: FakeAccount, body: Record<string, unknown>): Answer {
  const name = typeof body.display_name === "string" ? body.display_name.trim() : "";
  if (name === "" || name.length > 100) {
    return refuse(422, "invalid");
  }
  a.display_name = name;
  return { status: 200, body: { account: wire(a) } };
}

function registry(list: unknown[] | null, field: string): Answer {
  return list === null ? refuse(404, "not_found") : { status: 200, body: { [field]: list } };
}

function decline(a: FakeAccount, id: string): Answer {
  if (a.invites === null) {
    return refuse(404, "not_found");
  }
  a.invites = a.invites.filter((invite) => (invite as { id: string }).id !== id);
  return { status: 204 };
}

/** The endpoints that need a session; `undefined` when the path is not one of them. */
export function routeSignedIn(method: string, path: string, body: Record<string, unknown>, token: string | null): Answer | undefined {
  const session = token === null ? undefined : sessions.get(token);
  const a = session === undefined ? undefined : account(session.email);
  const key = `${method} ${path.replace(/\/(devices|invites)\/[^/]+/, "/$1/{id}")}`;
  const id = decodeURIComponent(path.split("/")[4] ?? "");
  const handlers: Record<string, (a: FakeAccount, current: string) => Answer> = {
    "GET /api/v1/me": (x) => ({ status: 200, body: { account: wire(x) } }),
    "PATCH /api/v1/me": (x) => rename(x, body),
    "GET /api/v1/devices": devices,
    "DELETE /api/v1/devices/{id}": (x) => revoke(x, id),
    "DELETE /api/v1/auth/sessions/current": (x, current) => revoke(x, current),
    "POST /api/v1/auth/password": (x, current) => changePassword(x, body, current),
    "GET /api/v1/invites": (x) => registry(x.invites, "invites"),
    "GET /api/v1/servers": (x) => registry(x.servers, "servers"),
    "POST /api/v1/invites/{id}/decline": (x) => decline(x, id),
  };
  const handler = handlers[key];
  if (handler === undefined) {
    return undefined;
  }
  return a === undefined || session === undefined ? refuse(401, "unauthorized") : handler(a, session.device);
}
