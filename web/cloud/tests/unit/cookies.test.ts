import { describe, expect, it } from "vitest";

import {
  FORGOT_FLOW_COOKIE,
  PRESESSION_COOKIE,
  SESSION_COOKIE,
  SESSION_SECONDS,
  SIGNUP_FLOW_COOKIE,
  cookieOptions,
} from "@/lib/server/cookie-options";
import { deriveKey } from "@/lib/server/seal";
import { openSession, readForgotFlow, readSignupFlow, sealSession, signFlow } from "@/lib/server/session";

const secret = new Uint8Array(32).fill(3);
const sessionKey = deriveKey(secret, "session");
const flowKey = deriveKey(secret, "flow");
const NOW = 1_800_000_000_000;

describe("cookie options", () => {
  it("are Secure, HttpOnly, SameSite=Lax and for the whole site", () => {
    expect(cookieOptions(10)).toEqual({ httpOnly: true, secure: true, sameSite: "lax", path: "/", maxAge: 10 });
  });

  it("name every cookie with the __Host- prefix, so none can carry a Domain", () => {
    for (const name of [SESSION_COOKIE, PRESESSION_COOKIE, SIGNUP_FLOW_COOKIE, FORGOT_FLOW_COOKIE]) {
      expect(name.startsWith("__Host-")).toBe(true);
    }
    expect(SESSION_COOKIE).toBe("__Host-orchester_session");
  });

  it("keep a web session 12 hours", () => {
    expect(SESSION_SECONDS).toBe(12 * 60 * 60);
  });
});

describe("session cookie", () => {
  const token = "ocd_secret_token_value";

  it("holds the token sealed, never in the clear", () => {
    const value = sealSession(sessionKey, token, NOW);
    expect(value).not.toContain(token);
    expect(Buffer.from(value, "base64url").toString("latin1")).not.toContain(token);
    expect(openSession(sessionKey, value, NOW)).toBe(token);
  });

  it("is refused after 12 hours, when changed, or under another key", () => {
    const value = sealSession(sessionKey, token, NOW);
    expect(openSession(sessionKey, value, NOW + SESSION_SECONDS * 1000)).toBeNull();
    const changed = `${value.slice(0, -2)}${value.endsWith("AA") ? "BA" : "AA"}`;
    expect(openSession(sessionKey, changed, NOW)).toBeNull();
    expect(openSession(deriveKey(secret, "flow"), value, NOW)).toBeNull();
    expect(openSession(sessionKey, undefined, NOW)).toBeNull();
    expect(openSession(sessionKey, "short", NOW)).toBeNull();
  });
});

describe("flow cookies", () => {
  const DAY = 24 * 60 * 60;

  it("read back what was signed, while they live", () => {
    const flow = { email: "a@example.com", sentAt: NOW, resent: true, limitedUntil: NOW + 5 };
    expect(readSignupFlow(flowKey, signFlow(flowKey, flow, NOW, DAY), NOW + 1000)).toEqual(flow);
    const forgot = signFlow(flowKey, { email: "a@example.com" }, NOW, 3600);
    expect(readForgotFlow(flowKey, forgot, NOW + 1000)).toEqual({ email: "a@example.com" });
  });

  it("are refused after the expiry they carry, even if the browser still sends them", () => {
    const flow = { email: "a@example.com", sentAt: NOW, resent: false };
    expect(readSignupFlow(flowKey, signFlow(flowKey, flow, NOW, DAY), NOW + DAY * 1000)).toBeNull();
    expect(readForgotFlow(flowKey, signFlow(flowKey, { email: "a@example.com" }, NOW, 3600), NOW + 3_600_000)).toBeNull();
  });

  it("refuse a changed address or another key", () => {
    const value = signFlow(flowKey, { email: "a@example.com" }, NOW, 3600);
    const [, signature] = value.split(".");
    const forged = `${Buffer.from(JSON.stringify({ email: "b@example.com", expires: NOW + 3_600_000 })).toString("base64url")}.${signature ?? ""}`;
    expect(readForgotFlow(flowKey, forged, NOW)).toBeNull();
    expect(readForgotFlow(sessionKey, value, NOW)).toBeNull();
  });
});
