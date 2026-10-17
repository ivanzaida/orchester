import type { Server } from "node:http";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { readConfig, ConfigError } from "@/lib/server/config";
import { declineInvite, devices, invites, revokeDevice, servers } from "@/lib/server/account";
import { confirmEmail, inspectLink, me, signIn, signUp } from "@/lib/server/cloud";
import { webDevice } from "@/lib/server/device";
import { t } from "@/lib/i18n";
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
    expect(result.ok && result.value.kind === "signedIn" && result.value.account).toEqual({
      email: ACCOUNT.email,
      displayName: ACCOUNT.display_name,
    });
    const last = (await recorded()).at(-1);
    expect(last?.path).toBe("/api/v1/auth/sessions");
    expect(last?.forwardedFor).toBe("203.0.113.7");
  });

  it("sends the session token as a bearer token", async () => {
    const signedIn = await signIn({ email: ACCOUNT.email, password: RIGHT_PASSWORD, device }, {});
    if (!signedIn.ok || signedIn.value.kind !== "signedIn") {
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
    expect(await confirmEmail("confirm-ok", "wrong password!", {})).toEqual({ ok: false, error: { kind: "refused", status: 401, code: "confirm_failed" } });
    expect(await confirmEmail("confirm-ok", RIGHT_PASSWORD, {})).toEqual({ ok: true, value: true });
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

describe("account api", () => {
  async function token(email: string): Promise<string> {
    const result = await signIn({ email, password: RIGHT_PASSWORD, device }, {});
    if (!result.ok || result.value.kind !== "signedIn") {
      throw new Error("sign-in failed");
    }
    return result.value.token;
  }

  it("reads devices, invites and servers in the page's shapes", async () => {
    const options = { token: await token(ACCOUNT.email) };
    const list = await devices(options);
    expect(list.ok && list.value.map((d) => [d.platform, d.current])).toEqual([["web", true], ["windows", false], ["macos", false]]);
    const open = await invites(options);
    expect(open.kind === "list" && open.items.map((invite) => [invite.kind, invite.role])).toEqual([
      ["project", "member"],
      ["server", null],
    ]);
    expect(open.kind === "list" && open.items[1]?.project).toBeNull();
    expect(open.kind === "list" && open.items[0]?.project?.name).toBe("mobile-app");
    const joined = await servers(options);
    expect(joined.kind === "list" && joined.items.map((s) => s.owner)).toEqual([false, true]);
  });

  it("names a section the cloud could not answer as failed, not as empty", async () => {
    const options = { token: "not a session" };
    expect(await invites(options)).toEqual({ kind: "failed" });
    expect(await servers(options)).toEqual({ kind: "failed" });
  });
});

describe("ids in API paths", () => {
  it("must be UUIDs, so a form cannot steer a request to another path", async () => {
    const options = { token: "anything" };
    const before = (await recorded()).length;
    for (const id of ["../me", "x/../../keys", "device-mbp", "%2e%2e"]) {
      expect(await revokeDevice(id, options)).toEqual({ ok: false, error: { kind: "refused", status: 400, code: "bad_request" } });
      expect(await declineInvite(id, options)).toEqual({ ok: false, error: { kind: "refused", status: 400, code: "bad_request" } });
    }
    expect((await recorded()).length).toBe(before);
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

  it("refuses a secret that is not strict base64, instead of reading what is left of it", () => {
    const loose = `${SECRET.slice(0, 20)}!!${SECRET.slice(20)}`;
    expect(() => readConfig({ ...good, ORCHESTER_CLOUD_WEB_SECRET: loose })).toThrow(/ORCHESTER_CLOUD_WEB_SECRET/);
    expect(() => readConfig({ ...good, ORCHESTER_CLOUD_WEB_SECRET: `${SECRET} ${SECRET}` })).toThrow(/base64/);
    expect(() => readConfig({ ...good, ORCHESTER_CLOUD_WEB_SECRET: SECRET.replace(/=+$/, "") })).toThrow(/base64/);
  });

  it("allows plain http only on this computer", () => {
    expect(readConfig({ ...good, ORCHESTER_CLOUD_PUBLIC_URL: "http://localhost:4100" }).publicOrigin).toBe("http://localhost:4100");
  });
});

describe("web device", () => {
  it("is named after the browser and the system", () => {
    const edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36 Edg/140.0";
    expect(webDevice(edge)).toEqual({ name: t("device.webName", { browser: "Edge", system: "Windows" }), platform: "web" });
    expect(webDevice(null)).toEqual({ name: t("device.unknownBrowser"), platform: "web" });
  });
});
