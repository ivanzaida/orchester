import type { Server } from "node:http";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { readConfig, ConfigError } from "@/lib/server/config";
import { confirmEmail, inspectLink, me, signIn, signUp } from "@/lib/server/cloud";
import { webDevice } from "@/lib/server/device";
import { failureMessage, waitMessage } from "@/lib/server/failures";

import { ACCOUNT, RIGHT_PASSWORD, startFakeApi } from "../fake-api/server";

const PORT = 4197;
const SECRET = Buffer.alloc(32, 9).toString("base64");
let server: Server;

beforeAll(() => {
  process.env.ORCHESTER_CLOUD_PUBLIC_URL = "http://localhost:4100";
  process.env.ORCHESTER_CLOUD_API_URL = `http://127.0.0.1:${PORT}`;
  process.env.ORCHESTER_CLOUD_WEB_SECRET = SECRET;
  server = startFakeApi(PORT);
});

afterAll(() => {
  server.close();
});

const device = webDevice(null);

async function recorded(): Promise<{ path: string; forwardedFor: string | null; authorization: string | null }[]> {
  const response = await fetch(`http://127.0.0.1:${PORT}/__fake/requests`);
  return (await response.json()) as { path: string; forwardedFor: string | null; authorization: string | null }[];
}

describe("api client", () => {
  it("reads a sign-in and sends the visitor's address", async () => {
    const result = await signIn({ email: ACCOUNT.email, password: RIGHT_PASSWORD, device }, { client: "203.0.113.7" });
    expect(result.ok && result.value.account).toEqual({ email: ACCOUNT.email, displayName: ACCOUNT.display_name });
    const last = (await recorded()).at(-1);
    expect(last?.path).toBe("/api/v1/auth/sessions");
    expect(last?.forwardedFor).toBe("203.0.113.7");
  });

  it("sends the session token as a bearer token", async () => {
    const signedIn = await signIn({ email: ACCOUNT.email, password: RIGHT_PASSWORD, device }, {});
    if (!signedIn.ok) {
      throw new Error("sign-in failed");
    }
    const account = await me({ token: signedIn.value.token });
    expect(account.ok).toBe(true);
    expect((await recorded()).at(-1)?.authorization).toBe(`Bearer ${signedIn.value.token}`);
  });

  it("types the cloud's refusals with their code and wait", async () => {
    expect(await signIn({ email: ACCOUNT.email, password: "wrong password!", device }, {})).toEqual({
      ok: false,
      error: { kind: "refused", status: 401, code: "invalid_credentials" },
    });
    expect(await signIn({ email: "limited@example.com", password: RIGHT_PASSWORD, device }, {})).toEqual({
      ok: false,
      error: { kind: "refused", status: 429, code: "rate_limited", retryAfter: 900 },
    });
    expect(await confirmEmail("nope", {})).toEqual({ ok: false, error: { kind: "refused", status: 422, code: "invalid_link" } });
    expect(await signUp({ email: "down@example.com", password: RIGHT_PASSWORD, displayName: "x" }, {})).toEqual({
      ok: false,
      error: { kind: "refused", status: 500, code: "internal" },
    });
  });

  it("reads the assumed link inspection", async () => {
    expect(await inspectLink("confirm-ok", "confirm", {})).toEqual({ ok: true, value: ACCOUNT.email });
    expect(await inspectLink("confirm-ok", "reset", {})).toMatchObject({ ok: false, error: { code: "invalid_link" } });
  });
});

describe("failure messages", () => {
  const now = Date.UTC(2026, 9, 15, 14, 32);

  it("say how long to wait and until when", () => {
    expect(waitMessage("signin.tooMany", 900, now)).toEqual({ key: "signin.tooMany", params: { minutes: 15 }, at: now + 900_000 });
    expect(waitMessage("signin.tooMany", 20, now).params).toEqual({ minutes: 1 });
  });

  it("say not answering for anything but a refused input", () => {
    expect(failureMessage({ kind: "unreachable" }, "error.tooManyEmails", now)).toEqual({ key: "error.unavailable" });
    expect(failureMessage({ kind: "unexpected", status: 200 }, "error.tooManyEmails", now)).toEqual({ key: "error.unavailable" });
    expect(failureMessage({ kind: "refused", status: 422, code: "invalid" }, "error.tooManyEmails", now)).toEqual({ key: "error.refused" });
    expect(failureMessage({ kind: "refused", status: 429, code: "rate_limited", retryAfter: 120 }, "error.tooManyEmails", now)).toEqual({
      key: "error.tooManyEmails",
      params: { minutes: 2 },
      at: now + 120_000,
    });
  });
});

describe("configuration", () => {
  const good = {
    ORCHESTER_CLOUD_PUBLIC_URL: "https://orchester-cloud.lurq.dev/",
    ORCHESTER_CLOUD_API_URL: "http://cloud:8080",
    ORCHESTER_CLOUD_WEB_SECRET: SECRET,
  };

  it("comes from the environment", () => {
    const config = readConfig(good);
    expect(config.publicOrigin).toBe("https://orchester-cloud.lurq.dev");
    expect(config.apiBase).toBe("http://cloud:8080/api/v1");
    expect(config.secret).toHaveLength(32);
  });

  it("names the variable that is wrong", () => {
    expect(() => readConfig({ ...good, ORCHESTER_CLOUD_PUBLIC_URL: "http://orchester-cloud.lurq.dev" })).toThrow(ConfigError);
    expect(() => readConfig({ ...good, ORCHESTER_CLOUD_WEB_SECRET: "c2hvcnQ=" })).toThrow(/ORCHESTER_CLOUD_WEB_SECRET/);
    expect(() => readConfig({ ...good, ORCHESTER_CLOUD_API_URL: "" })).toThrow(/ORCHESTER_CLOUD_API_URL/);
  });

  it("allows plain http only on this computer", () => {
    expect(readConfig({ ...good, ORCHESTER_CLOUD_PUBLIC_URL: "http://localhost:4100" }).publicOrigin).toBe("http://localhost:4100");
  });
});

describe("web device", () => {
  it("is named after the browser and the system", () => {
    const edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36 Edg/140.0";
    expect(webDevice(edge)).toEqual({ name: "Edge on Windows", platform: "web" });
    expect(webDevice(null)).toEqual({ name: "Web browser", platform: "web" });
  });
});
