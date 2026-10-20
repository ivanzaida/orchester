import type { Server } from "node:http";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import type { CallOptions } from "@/lib/server/api";
import { failureMessage } from "@/lib/server/failures";
import { approvals, checkSetupCode, me, serverInfo, signInWithCloud, signInWithCode } from "@/lib/server/team/auth";
import { createInvite, invites, revokeInvite } from "@/lib/server/team/invites";
import { devices, people, revokeDevice, updatePerson } from "@/lib/server/team/people";
import { managedProjects, members, removeMember, setRole } from "@/lib/server/team/projects";
import { cloudLink, settings } from "@/lib/server/team/settings";

import { identityToken } from "../fake-api/cloud";
import { startFakeApi } from "../fake-api/server";

const PORT = 4297;
let server: Server;

beforeAll(() => {
  process.env.ORCHESTER_ADMIN_SERVER_URL = `http://127.0.0.1:${PORT}`;
  process.env.ORCHESTER_ADMIN_SECRET = Buffer.alloc(32, 9).toString("base64");
  server = startFakeApi(PORT);
});

afterAll(() => {
  server.close();
});

const place = { proto: "https" as const, host: "orchester.kontur.dev" };
const anonymous: CallOptions = { place, client: "203.0.113.7" };

async function recorded(): Promise<{ path: string; forwardedFor: string | null; forwardedHost: string | null; forwardedProto: string | null; authorization: string | null }[]> {
  const response = await fetch(`http://127.0.0.1:${PORT}/__fake/requests`);
  return (await response.json()) as never;
}

async function signedIn(code = "ADMN-0001"): Promise<CallOptions> {
  const result = await signInWithCode({ code, device: "Edge on Windows" }, anonymous);
  if (!result.ok) {
    throw new Error("sign-in failed");
  }
  return { ...anonymous, token: result.value.token };
}

describe("the team server's API", () => {
  it("reads the server and passes on where the browser is", async () => {
    const info = await serverInfo(anonymous);
    expect(info.ok && info.value).toMatchObject({ name: "Kontur team", needsSetup: false, certificate: { mode: "self_signed" } });
    const last = (await recorded()).at(-1);
    expect(last).toMatchObject({ path: "/api/v1/server", forwardedFor: "203.0.113.7", forwardedHost: "orchester.kontur.dev", forwardedProto: "https" });
  });

  it("signs in with a code as an admin browser and sends the token as a bearer", async () => {
    const options = await signedIn();
    const user = await me(options);
    expect(user.ok && user.value.username).toBe("ivan");
    expect((await recorded()).at(-1)?.authorization).toBe(`Bearer ${options.token ?? ""}`);
  });

  it("types the refusals a sign-in meets", async () => {
    expect(await signInWithCode({ code: "WRNG-0000", device: "x" }, anonymous)).toEqual({ ok: false, error: { kind: "refused", status: 401, code: "invalid_code" } });
    expect(await signInWithCode({ code: "NONE-0001", device: "x" }, anonymous)).toMatchObject({ ok: false, error: { code: "nothing_to_manage" } });
    expect(await signInWithCode({ code: "LIMT-0001", device: "x" }, anonymous)).toEqual({ ok: false, error: { kind: "refused", status: 429, code: "rate_limited", retryAfter: 900 } });
    expect(await checkSetupCode("Q4ZD-8M2K", anonymous)).toMatchObject({ ok: false, error: { code: "invalid_code" } });
    const stranger = await signInWithCloud({ identityToken: identityToken("stranger@example.com", "S"), keySet: "k", device: "x" }, anonymous);
    expect(stranger).toMatchObject({ ok: false, error: { code: "not_invited" } });
  });

  it("reads every list a page shows in the model's shapes", async () => {
    const options = await signedIn();
    const projects = await managedProjects(options);
    expect(projects.ok && projects.value.map((p) => [p.slug, p.role, p.address])).toEqual([
      ["kontur", "owner", "https://orchester.kontur.dev/kontur"],
      ["billing", "owner", "https://orchester.kontur.dev/billing"],
      ["mobile-app", null, "https://orchester.kontur.dev/mobile-app"],
    ]);
    const kontur = projects.ok ? projects.value[0] : undefined;
    expect((await members(kontur?.id ?? "", options)).ok).toBe(true);
    const list = await invites(kontur?.id ?? null, options);
    expect(list.ok && list.value.filter((i) => i.state !== "open").every((i) => i.email === null)).toBe(true);
    for (const result of [await people(options), await devices(options), await approvals(options), await settings(options), await cloudLink(options)]) {
      expect(result.ok).toBe(true);
    }
  });

  it("answers the last owner and the last admin as refusals the pages name", async () => {
    const options = await signedIn("OWNR-0001");
    const projects = await managedProjects(options);
    const mobile = projects.ok ? projects.value.find((p) => p.slug === "mobile-app") : undefined;
    const maria = "0199f0a0-0000-7000-8000-000000000004";
    expect(await setRole(mobile?.id ?? "", maria, "member", options)).toMatchObject({ ok: false, error: { code: "last_owner" } });
    expect(await removeMember(mobile?.id ?? "", maria, options)).toMatchObject({ ok: false, error: { code: "last_owner" } });
  });

  it("makes a code invite and shows its code once, and revokes", async () => {
    const options = await signedIn();
    const made = await createInvite({ kind: "server", delivery: "code", admin: false }, options);
    expect(made.ok && made.value.code).toBe("K7QM-3XRD");
    expect(made.ok && made.value.signed).toBeNull();
    const revoked = await revokeInvite(made.ok ? made.value.invite.id : "", options);
    expect(revoked).toEqual({ ok: true, value: { revocation: null } });
  });

  it("reads a repeated invite by email as a refusal with when the open one was sent", async () => {
    const options = await signedIn();
    const input = { kind: "server" as const, delivery: "cloud" as const, admin: false, email: "twice@example.com" };
    const first = await createInvite(input, options);
    const again = await createInvite(input, options);
    expect(again).toEqual({
      ok: false,
      error: { kind: "refused", status: 409, code: "invite_open", sentAt: first.ok ? first.value.invite.createdAt : -1 },
    });
    const member = await createInvite({ ...input, email: "member.bob@example.com" }, options);
    expect(member).toEqual({ ok: false, error: { kind: "refused", status: 409, code: "already_member" } });
  });
});

describe("ids in API paths", () => {
  it("must be UUIDs, so a form cannot steer a request to another path", async () => {
    const options = { ...anonymous, token: "anything" };
    const before = (await recorded()).length;
    for (const id of ["../server", "x/../../admin/users", "%2e%2e"]) {
      expect(await revokeDevice(id, options)).toEqual({ ok: false, error: { kind: "refused", status: 400, code: "bad_request" } });
      expect(await revokeInvite(id, options)).toMatchObject({ ok: false, error: { code: "bad_request" } });
      expect(await updatePerson(id, { admin: true }, options)).toMatchObject({ ok: false, error: { code: "bad_request" } });
      expect(await members(id, options)).toMatchObject({ ok: false, error: { code: "bad_request" } });
    }
    expect((await recorded()).length).toBe(before);
  });
});

describe("failure messages", () => {
  const now = Date.UTC(2026, 9, 15, 14, 32);

  it("say not answering for anything but a refused input, a refusal of rights or a limit", () => {
    expect(failureMessage({ kind: "unreachable" }, now)).toEqual({ key: "error.unavailable" });
    expect(failureMessage({ kind: "unexpected", status: 200 }, now)).toEqual({ key: "error.unavailable" });
    expect(failureMessage({ kind: "refused", status: 422, code: "invalid" }, now)).toEqual({ key: "error.refused" });
    expect(failureMessage({ kind: "refused", status: 403, code: "forbidden" }, now)).toEqual({ key: "error.forbidden" });
    expect(failureMessage({ kind: "refused", status: 429, code: "rate_limited", retryAfter: 120 }, now)).toEqual({
      key: "error.tooManyTries",
      params: { minutes: 2 },
      at: now + 120_000,
    });
  });
});
