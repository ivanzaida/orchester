import type { Server } from "node:http";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { invites } from "@/lib/server/account";
import { signIn } from "@/lib/server/cloud";
import { CHALLENGE_COOKIE, CHALLENGE_SECONDS } from "@/lib/server/cookie-options";
import { webDevice } from "@/lib/server/device";
import { deriveKey } from "@/lib/server/seal";
import { openChallenge, sealChallenge } from "@/lib/server/session";
import { account, confirmSetup, finishSignIn, qrCode, startSetup, stepUp, turnOff } from "@/lib/server/two-factor";
import { recoveryCodesText } from "@/lib/two-factor";

import { APP_CODE, RECOVERY_CODE, TWO_FACTOR } from "../fake-api/two-factor";
import { ACCOUNT, RIGHT_PASSWORD, startFakeApi } from "../fake-api/server";

let server: Server;
const device = webDevice(null);

beforeAll(async () => {
  const started = await startFakeApi();
  server = started.server;
  process.env.ORCHESTER_CLOUD_PUBLIC_URL = "http://localhost:4100";
  process.env.ORCHESTER_CLOUD_API_URL = started.origin;
  process.env.ORCHESTER_CLOUD_WEB_SECRET = Buffer.alloc(32, 9).toString("base64");
});

afterAll(() => {
  server.close();
});

async function token(email: string): Promise<string> {
  const result = await signIn({ email, password: RIGHT_PASSWORD, device }, {});
  if (!result.ok || result.value.kind !== "signedIn") {
    throw new Error("sign-in failed");
  }
  return result.value.token;
}

describe("two-factor sign-in", () => {
  it("reads the password step's challenge, and its code signs in", async () => {
    const answer = await signIn({ email: TWO_FACTOR, password: RIGHT_PASSWORD, device }, {});
    if (!answer.ok || answer.value.kind !== "challenged") {
      throw new Error("a challenge was expected");
    }
    expect(answer.value.expiresAt).toBeGreaterThan(Date.now());
    const wrong = await finishSignIn(answer.value.challenge, "000000", {});
    expect(wrong).toEqual({ ok: false, error: { kind: "refused", status: 401, code: "invalid_code" } });
    const done = await finishSignIn(answer.value.challenge, RECOVERY_CODE, {});
    expect(done.ok && done.value.recoveryCodesLeft).toBe(9);
    expect(done.ok && done.value.account.email).toBe(TWO_FACTOR);
    const again = await finishSignIn(answer.value.challenge, APP_CODE, {});
    expect(again).toMatchObject({ ok: false, error: { code: "challenge_expired" } });
  });
});

describe("two-factor setup", () => {
  it("turns it on with the password and a code, and the session and invites say so", async () => {
    const options = { token: await token(ACCOUNT.email) };
    const before = await account(options);
    expect(before.ok && before.value.twoFactor).toEqual({ enabled: false, enabledAt: null, recoveryCodesLeft: 0, thisSession: false });
    const listed = await invites(options);
    expect(listed.kind === "list" && listed.items.every((invite) => invite.needsTwoFactor)).toBe(true);
    expect(await startSetup("not the password", options)).toMatchObject({ ok: false, error: { code: "invalid_credentials" } });
    const setup = await startSetup(RIGHT_PASSWORD, options);
    expect(setup.ok && setup.value.uri.startsWith("otpauth://totp/")).toBe(true);
    const codes = await confirmSetup(APP_CODE, options);
    expect(codes.ok && codes.value.length).toBe(10);
    const after = await account(options);
    expect(after.ok && after.value.twoFactor).toMatchObject({ enabled: true, recoveryCodesLeft: 10, thisSession: true });
    const relisted = await invites(options);
    expect(relisted.kind === "list" && relisted.items.some((invite) => invite.needsTwoFactor)).toBe(false);
    expect(await stepUp(APP_CODE, options)).toEqual({ ok: true, value: 10 });
    expect(await turnOff(RIGHT_PASSWORD, "000000", options)).toMatchObject({ ok: false, error: { code: "invalid_code" } });
    expect(await turnOff(RIGHT_PASSWORD, APP_CODE, options)).toEqual({ ok: true, value: true });
  });
});

describe("the QR code", () => {
  it("is one path of whole modules inside a quiet zone of four", () => {
    const code = qrCode("otpauth://totp/cloud.example.test:mira%40example.com?secret=JBSWY3DPEHPK3PXP&issuer=cloud.example.test");
    expect((code.size - 8 - 21) % 4).toBe(0);
    const squares = code.path.match(/M(\d+) (\d+)h1v1h-1z/g) ?? [];
    expect(squares.length).toBeGreaterThan(100);
    expect(squares.join("")).toBe(code.path);
    const corners = squares.map((square) => square.slice(1, -8).split(" ").map(Number));
    expect(corners.every(([x, y]) => x !== undefined && y !== undefined && x >= 4 && y >= 4 && x < code.size - 4 && y < code.size - 4)).toBe(true);
  });
});

describe("the challenge cookie", () => {
  const key = deriveKey(new Uint8Array(32).fill(5), "challenge");
  const NOW = 1_800_000_000_000;
  const kept = { challenge: "orcm_fake_challenge_secret", email: TWO_FACTOR, next: null };

  it("holds the challenge sealed for five minutes, and only for its own key", () => {
    expect(CHALLENGE_COOKIE.startsWith("__Host-")).toBe(true);
    const value = sealChallenge(key, kept, NOW);
    expect(value).not.toContain("orcm_");
    expect(openChallenge(key, value, NOW + 1000)).toEqual(kept);
    expect(openChallenge(key, value, NOW + CHALLENGE_SECONDS * 1000)).toBeNull();
    expect(openChallenge(deriveKey(new Uint8Array(32).fill(5), "session"), value, NOW)).toBeNull();
  });

  it("writes recovery codes one a line", () => {
    expect(recoveryCodesText(["abcde-fghjk", "bcdef-ghjkm"])).toBe("abcde-fghjk\nbcdef-ghjkm\n");
  });
});
