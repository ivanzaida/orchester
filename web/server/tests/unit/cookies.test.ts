import { describe, expect, it } from "vitest";

import {
  NOTHING_COOKIE,
  PRESESSION_COOKIE,
  SESSION_COOKIE,
  SESSION_SECONDS,
  SETUP_COOKIE,
  SETUP_SECONDS,
  STATE_COOKIE,
  STATE_SECONDS,
  cookieOptions,
  crossSiteCookieOptions,
} from "@/lib/server/cookie-options";
import { deriveKey } from "@/lib/server/seal";
import {
  openSession,
  openSetup,
  openTrip,
  readNothing,
  sealSession,
  sealSetup,
  sealTrip,
  signNothing,
  tripMatches,
  type Trip,
} from "@/lib/server/session";

const secret = new Uint8Array(32).fill(3);
const sessionKey = deriveKey(secret, "session");
const setupKey = deriveKey(secret, "setup");
const stateKey = deriveKey(secret, "state");
const flowKey = deriveKey(secret, "flow");
const NOW = 1_800_000_000_000;

describe("cookie options", () => {
  it("are Secure, HttpOnly, for the whole site; Lax, except the trip's state", () => {
    expect(cookieOptions(10)).toEqual({ httpOnly: true, secure: true, sameSite: "lax", path: "/", maxAge: 10 });
    expect(crossSiteCookieOptions(10)).toEqual({ httpOnly: true, secure: true, sameSite: "none", path: "/", maxAge: 10 });
  });

  it("name every cookie with the __Host- prefix and never as web/cloud's", () => {
    const names = [SESSION_COOKIE, PRESESSION_COOKIE, STATE_COOKIE, SETUP_COOKIE, NOTHING_COOKIE];
    for (const name of names) {
      expect(name.startsWith("__Host-orchester_admin")).toBe(true);
    }
    expect(names).not.toContain("__Host-orchester_session");
    expect(names).not.toContain("__Host-orchester_presession");
    expect(SESSION_COOKIE).toBe("__Host-orchester_admin");
    expect(STATE_COOKIE).toBe("__Host-orchester_admin_state");
    expect(SETUP_COOKIE).toBe("__Host-orchester_admin_setup");
  });

  it("live 12 hours for a session, 15 minutes for the setup code, 10 for a trip", () => {
    expect(SESSION_SECONDS).toBe(12 * 60 * 60);
    expect(SETUP_SECONDS).toBe(15 * 60);
    expect(STATE_SECONDS).toBe(10 * 60);
  });
});

describe("session cookie", () => {
  const session = { token: "adm_secret_token_value", via: "cloud" as const };

  it("holds the token sealed, never in the clear, with how the person signed in", () => {
    const value = sealSession(sessionKey, session, NOW);
    expect(value).not.toContain(session.token);
    expect(Buffer.from(value, "base64url").toString("latin1")).not.toContain(session.token);
    expect(openSession(sessionKey, value, NOW)).toEqual(session);
  });

  it("is refused after 12 hours, when changed, or under another key", () => {
    const value = sealSession(sessionKey, session, NOW);
    expect(openSession(sessionKey, value, NOW + SESSION_SECONDS * 1000)).toBeNull();
    const changed = `${value.slice(0, -2)}${value.endsWith("AA") ? "BA" : "AA"}`;
    expect(openSession(sessionKey, changed, NOW)).toBeNull();
    expect(openSession(setupKey, value, NOW)).toBeNull();
    expect(openSession(sessionKey, undefined, NOW)).toBeNull();
    expect(openSession(sessionKey, "short", NOW)).toBeNull();
  });
});

describe("setup cookie", () => {
  it("keeps the setup code sealed for 15 minutes", () => {
    const value = sealSetup(setupKey, "Q4ZD-8M2K", NOW);
    expect(value).not.toContain("Q4ZD");
    expect(openSetup(setupKey, value, NOW + 60_000)).toBe("Q4ZD-8M2K");
    expect(openSetup(setupKey, value, NOW + SETUP_SECONDS * 1000)).toBeNull();
    expect(openSetup(sessionKey, value, NOW)).toBeNull();
  });
});

describe("trip cookie", () => {
  const admin: Trip = { state: "s-1", purpose: "admin", back: "signin", session: null };
  const link: Trip = { state: "s-2", purpose: "link", back: "setup", session: { token: "adm_link_token", via: "code" } };

  it("reads back its state, purpose and where it goes back, for 10 minutes", () => {
    expect(openTrip(stateKey, sealTrip(stateKey, admin, NOW), NOW + 1000)).toEqual(admin);
    expect(openTrip(stateKey, sealTrip(stateKey, admin, NOW), NOW + STATE_SECONDS * 1000)).toBeNull();
  });

  it("carries a link's session sealed, unreadable in the browser", () => {
    const value = sealTrip(stateKey, link, NOW);
    expect(value).not.toContain("adm_link_token");
    expect(Buffer.from(value, "base64url").toString("latin1")).not.toContain("adm_link_token");
    expect(openTrip(stateKey, value, NOW)).toEqual(link);
    expect(openTrip(sessionKey, value, NOW)).toBeNull();
  });

  it("matches only its own state", () => {
    expect(tripMatches(admin, "s-1")).toBe(true);
    expect(tripMatches(admin, "s-2")).toBe(false);
    expect(tripMatches(admin, null)).toBe(false);
    expect(tripMatches(admin, new File([], "s-1"))).toBe(false);
  });
});

describe("nothing-to-manage cookie", () => {
  it("reads back the name and email while it lives, and refuses a changed one", () => {
    const value = signNothing(flowKey, { name: "Dana Ruiz", email: "dana@example.com" }, NOW);
    expect(readNothing(flowKey, value, NOW + 1000)).toEqual({ name: "Dana Ruiz", email: "dana@example.com" });
    expect(readNothing(flowKey, value, NOW + 11 * 60_000)).toBeNull();
    const [, signature] = value.split(".");
    const forged = `${Buffer.from(JSON.stringify({ name: "X", email: "x@example.com", expires: NOW + 60_000 })).toString("base64url")}.${signature ?? ""}`;
    expect(readNothing(flowKey, forged, NOW)).toBeNull();
  });
});
