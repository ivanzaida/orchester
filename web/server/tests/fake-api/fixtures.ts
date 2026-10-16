/**
 * The fake team server's data: the design's illustrative Kontur team (`FCQCr` and its states). Fixture data for the
 * page tests and the screenshots only; nothing here ships. Times are relative to when the state was made.
 */

export const HOUR = 60 * 60 * 1000;
export const DAY = 24 * HOUR;

export interface FakeUser {
  id: string;
  username: string;
  display_name: string;
  admin: boolean;
  disabled: boolean;
  cloud: boolean;
  email: string | null;
}

export interface FakeMember {
  user: string;
  role: "owner" | "member";
  added_at: number;
}

export interface FakeProject {
  id: string;
  slug: string;
  key: string;
  name: string;
  remote_hint: string | null;
  created_at: number;
  created_by: string;
  members: FakeMember[];
}

export interface FakeInvite {
  id: string;
  kind: "project" | "server" | "account";
  email: string | null;
  project: string | null;
  role: "owner" | "member" | null;
  user: string | null;
  admin: boolean;
  by: string;
  delivery: "cloud" | "code";
  expires_at: number;
  created_at: number;
  state: "open" | "accepted" | "expired" | "revoked";
  ended_at: number | null;
  ended_by: string | null;
}

export interface FakeDevice {
  id: string;
  device: string;
  user: string;
  via: "cloud" | "code" | "before_3";
  created_at: number;
  last_used_at: number;
  expires_at: number;
}

export interface FakeApproval {
  id: string;
  user: string;
  device: string;
  address: string | null;
  requested_at: number;
  expires_at: number;
}

export interface FakeCloud {
  server: string;
  root_fingerprint: string | null;
  accepting: boolean;
  key_set_issued_at: number | null;
  key_set_expires_at: number | null;
  linked_at: number | null;
  linked_by: string | null;
}

export interface FakeState {
  name: string;
  needs_setup: boolean;
  version: string;
  join_approval: boolean;
  certificate: { check_code: string; sha256: string; mode: "self_signed" | "pem" } | null;
  cloud: FakeCloud | null;
  users: FakeUser[];
  projects: FakeProject[];
  invites: FakeInvite[];
  devices: FakeDevice[];
  approvals: FakeApproval[];
}

export const id = (n: number) => `0199f0a0-0000-7000-8000-${String(n).padStart(12, "0")}`;

export const IVAN = id(1);
export const ALICE = id(2);
export const OLEH = id(3);
export const MARIA = id(4);
export const DANA = id(5);
export const KONTUR = id(11);
export const BILLING = id(12);
export const MOBILE = id(13);
export const SERVER_ID = id(99);

/** The setup code the fake server's console would print. */
export const SETUP_CODE = "Q4ZD-8M2K";

const CERTIFICATE = {
  check_code: "3F7A-92C1-KD8M-QX4T",
  sha256: "4f7a92c10d8e51b3a91c7e2d4b6f8a0c1e3d5f7092b4c6d8e0f1a2b36a2fc091",
  mode: "self_signed" as const,
};

function users(): FakeUser[] {
  return [
    { id: IVAN, username: "ivan", display_name: "Ivan Zaida", admin: true, disabled: false, cloud: true, email: "ivan@example.com" },
    { id: ALICE, username: "alice", display_name: "Alice Moreau", admin: true, disabled: false, cloud: true, email: "alice@example.com" },
    { id: OLEH, username: "oleh", display_name: "Oleh Petrenko", admin: false, disabled: false, cloud: false, email: null },
    { id: MARIA, username: "maria", display_name: "Maria Lind", admin: false, disabled: false, cloud: true, email: "maria@example.com" },
    { id: DANA, username: "dana", display_name: "Dana Ruiz", admin: false, disabled: true, cloud: true, email: "dana@example.com" },
  ];
}

function projects(now: number): FakeProject[] {
  return [
    {
      id: KONTUR, slug: "kontur", key: "KTR", name: "Kontur", remote_hint: "github.com/acme/kontur", created_at: now - 14 * DAY, created_by: IVAN,
      members: [
        { user: IVAN, role: "owner", added_at: now - 14 * DAY },
        { user: ALICE, role: "owner", added_at: now - 14 * DAY },
        { user: OLEH, role: "member", added_at: now - 13 * DAY },
        { user: MARIA, role: "member", added_at: now - 10 * DAY },
      ],
    },
    {
      id: BILLING, slug: "billing", key: "BIL", name: "Billing", remote_hint: "github.com/acme/billing", created_at: now - 10 * DAY, created_by: IVAN,
      members: [
        { user: IVAN, role: "owner", added_at: now - 10 * DAY },
        { user: ALICE, role: "member", added_at: now - 9 * DAY },
        { user: MARIA, role: "member", added_at: now - 8 * DAY },
      ],
    },
    {
      id: MOBILE, slug: "mobile-app", key: "MOB", name: "Mobile app", remote_hint: null, created_at: now - 5 * DAY, created_by: ALICE,
      members: [
        { user: MARIA, role: "owner", added_at: now - 5 * DAY },
        { user: ALICE, role: "member", added_at: now - 5 * DAY },
      ],
    },
  ];
}

type InviteSeed = Pick<FakeInvite, "id" | "email" | "project" | "role" | "by" | "delivery" | "state"> & Partial<FakeInvite> & { age: number };

function invite(now: number, seed: InviteSeed): FakeInvite {
  const created = now - seed.age;
  const life = seed.delivery === "code" ? 3 * DAY : 7 * DAY;
  return {
    kind: "project", user: null, admin: false, expires_at: created + life, created_at: created, ended_at: null, ended_by: null,
    ...seed,
  };
}

function invites(now: number): FakeInvite[] {
  return [
    invite(now, { id: id(21), email: "dana@example.com", project: KONTUR, role: "member", by: IVAN, delivery: "cloud", state: "open", age: 2 * HOUR }),
    invite(now, { id: id(22), email: "paul@example.com", project: KONTUR, role: "owner", by: ALICE, delivery: "cloud", state: "open", age: 1 * HOUR }),
    invite(now, { id: id(23), email: null, project: KONTUR, role: "member", by: IVAN, delivery: "code", state: "open", age: 3 * HOUR }),
    invite(now, { id: id(24), email: null, project: KONTUR, role: "member", by: IVAN, delivery: "cloud", state: "expired", age: 12 * DAY, ended_at: now - 5 * DAY }),
    invite(now, { id: id(25), email: null, project: KONTUR, role: "member", by: ALICE, delivery: "cloud", state: "revoked", age: 6 * DAY, ended_at: now - 4 * DAY, ended_by: ALICE }),
    invite(now, { id: id(26), email: null, project: KONTUR, role: "member", by: IVAN, delivery: "cloud", state: "accepted", age: 11 * DAY, ended_at: now - 10 * DAY, ended_by: MARIA }),
    invite(now, { id: id(27), email: "nina@example.com", project: MOBILE, role: "member", by: MARIA, delivery: "cloud", state: "open", age: 4 * HOUR }),
    invite(now, { id: id(28), email: null, project: MOBILE, role: "member", by: MARIA, delivery: "code", state: "open", age: 5 * HOUR }),
    invite(now, { id: id(29), kind: "server", email: "lena@example.com", project: null, role: null, admin: true, by: IVAN, delivery: "cloud", state: "open", age: 1 * DAY }),
  ];
}

function devices(now: number): FakeDevice[] {
  const device = (n: number, name: string, user: string, via: FakeDevice["via"], joined: number, used: number): FakeDevice => ({
    id: id(n), device: name, user, via, created_at: now - joined, last_used_at: now - used, expires_at: now - used + 90 * DAY,
  });
  return [
    device(31, "IVAN-PC", IVAN, "cloud", 14 * DAY, 30 * 1000),
    device(32, "ivan-mbp", IVAN, "cloud", 12 * DAY, 2 * DAY + 3 * HOUR),
    device(33, "alice-desktop", ALICE, "cloud", 14 * DAY, 2 * HOUR),
    device(34, "oleh-laptop", OLEH, "code", 13 * DAY, 4 * HOUR),
    device(35, "maria-mbp", MARIA, "cloud", 10 * DAY, 1 * DAY),
    device(36, "lab-box", MARIA, "code", 7 * DAY, 6 * DAY),
    device(37, "build-02", ALICE, "before_3", 105 * DAY, 85 * DAY),
  ];
}

export function fullState(now: number): FakeState {
  return {
    name: "Kontur team",
    needs_setup: false,
    version: "0.4.0",
    join_approval: true,
    certificate: CERTIFICATE,
    cloud: {
      server: SERVER_ID, root_fingerprint: "nQ3hW8vK2pXcT5yLmR7aZ1bF9dG4sJ6e", accepting: true,
      key_set_issued_at: now - 5 * HOUR, key_set_expires_at: now + 29 * DAY, linked_at: now - 14 * DAY, linked_by: IVAN,
    },
    users: users(),
    projects: projects(now),
    invites: invites(now),
    devices: devices(now),
    approvals: [{ id: id(41), user: OLEH, device: "oleh-pc", address: "10.0.4.31", requested_at: now - 20 * 60 * 1000, expires_at: now + 10 * DAY }],
  };
}

/** The scenarios a test asks for with `POST /__fake/reset { scenario }`. */
export function scenario(name: string, now: number): FakeState {
  const state = fullState(now);
  switch (name) {
    case "setup":
      return { ...state, needs_setup: true, name: "Orchester server", cloud: null, users: [], projects: [], invites: [], devices: [], approvals: [] };
    case "empty":
      return { ...state, projects: [], invites: [], approvals: [] };
    case "unlinked":
      return { ...state, cloud: null, invites: state.invites.filter((invite) => invite.delivery === "code") };
    case "cloud-off":
      return { ...state, cloud: state.cloud === null ? null : { ...state.cloud, accepting: false } };
    case "stale":
      return { ...state, cloud: state.cloud === null ? null : { ...state.cloud, key_set_issued_at: now - 34 * DAY, key_set_expires_at: now - 4 * DAY } };
    case "no-approval":
      return { ...state, join_approval: false, approvals: [] };
    case "plain":
      return { ...state, certificate: null };
    default:
      return state;
  }
}
