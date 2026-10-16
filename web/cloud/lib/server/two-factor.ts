import qrcode from "qrcode-generator";

import { call, nothing, type ApiResult, type CallOptions } from "@/lib/server/api";
import { readAccount, readSignedIn, record, type Account, type SignedIn } from "@/lib/server/cloud";
import type { QrCode, TwoFactorStatus } from "@/lib/two-factor";

/**
 * The cloud's two-factor endpoints (`docs/cloud-api.md`, "Two-factor authentication"): the sign-in's second step,
 * stepping a session up, turning it on and off, and new recovery codes. Codes are passed as typed; the cloud reads
 * spaces, dashes and case itself.
 */

export interface Setup {
  readonly secret: string;
  readonly uri: string;
}

export function readTwoFactor(json: unknown): TwoFactorStatus | null {
  const { enabled, enabled_at: enabledAt, recovery_codes_left: left, this_session: thisSession } = record(json) ?? {};
  if (typeof enabled !== "boolean" || typeof left !== "number" || typeof thisSession !== "boolean") {
    return null;
  }
  if (enabledAt !== null && typeof enabledAt !== "number") {
    return null;
  }
  return { enabled, enabledAt, recoveryCodesLeft: left, thisSession };
}

function readCodes(json: unknown): string[] | null {
  const codes = record(json)?.recovery_codes;
  return Array.isArray(codes) && codes.every((code) => typeof code === "string") ? codes : null;
}

function readLeft(json: unknown): number | null {
  const left = record(json)?.recovery_codes_left;
  return typeof left === "number" ? left : null;
}

/** `GET /me` with what it says of two-factor authentication to this browser. */
export function account(options: CallOptions): Promise<ApiResult<{ account: Account; twoFactor: TwoFactorStatus }>> {
  const read = (json: unknown) => {
    const found = readAccount(record(json)?.account);
    const twoFactor = readTwoFactor(record(json)?.two_factor);
    return found === null || twoFactor === null ? null : { account: found, twoFactor };
  };
  return call("GET", "/me", { expected: 200, read }, options);
}

/** `POST /auth/sessions/two-factor`: the challenge of the password step and a code sign this browser in. */
export function finishSignIn(
  challenge: string,
  code: string,
  options: CallOptions,
): Promise<ApiResult<SignedIn & { recoveryCodesLeft: number }>> {
  const read = (json: unknown) => {
    const signedIn = readSignedIn(json);
    const left = readLeft(json);
    return signedIn === null || left === null ? null : { ...signedIn, recoveryCodesLeft: left };
  };
  return call("POST", "/auth/sessions/two-factor", { body: { challenge, code }, expected: 201, read }, options);
}

/** `POST /auth/sessions/current/two-factor`: a code makes this browser's session a two-factor one. */
export function stepUp(code: string, options: CallOptions): Promise<ApiResult<number>> {
  return call("POST", "/auth/sessions/current/two-factor", { body: { code }, expected: 200, read: readLeft }, options);
}

export function startSetup(password: string, options: CallOptions): Promise<ApiResult<Setup>> {
  const read = (json: unknown) => {
    const { secret, uri } = record(json) ?? {};
    return typeof secret === "string" && typeof uri === "string" ? { secret, uri } : null;
  };
  return call("POST", "/me/two-factor", { body: { password }, expected: 201, read }, options);
}

export function confirmSetup(code: string, options: CallOptions): Promise<ApiResult<string[]>> {
  return call("POST", "/me/two-factor/confirm", { body: { code }, expected: 200, read: readCodes }, options);
}

export function turnOff(password: string, code: string, options: CallOptions): Promise<ApiResult<true>> {
  return call("DELETE", "/me/two-factor", { body: { password, code }, expected: 204, read: nothing }, options);
}

export function newRecoveryCodes(code: string, options: CallOptions): Promise<ApiResult<string[]>> {
  return call("POST", "/me/two-factor/recovery-codes", { body: { code }, expected: 200, read: readCodes }, options);
}

/** The modules a QR code keeps clear around itself, so a camera finds its edges. */
const QUIET_ZONE = 4;

/**
 * The QR code of the `otpauth://` link an authenticator app reads, made here so the page needs no script for it:
 * error correction M, the smallest version that holds it.
 */
export function qrCode(uri: string): QrCode {
  const code = qrcode(0, "M");
  code.addData(uri, "Byte");
  code.make();
  const count = code.getModuleCount();
  const squares: string[] = [];
  for (let row = 0; row < count; row += 1) {
    for (let column = 0; column < count; column += 1) {
      if (code.isDark(row, column)) {
        squares.push(`M${column + QUIET_ZONE} ${row + QUIET_ZONE}h1v1h-1z`);
      }
    }
  }
  return { size: count + 2 * QUIET_ZONE, path: squares.join("") };
}
