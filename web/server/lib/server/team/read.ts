import type { Certificate, Role, User, UserRef } from "@/lib/model";

/**
 * Reading the team server's JSON into the admin's model (`lib/model.ts`). Each reader answers `null` for anything
 * without the contract's shape, which the API client turns into an `unexpected` answer.
 */

export type Json = Record<string, unknown>;

export function record(json: unknown): Json | null {
  return typeof json === "object" && json !== null && !Array.isArray(json) ? (json as Json) : null;
}

export function text(value: unknown): value is string {
  return typeof value === "string";
}

export function number(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

export function textOrNull(value: unknown): string | null | undefined {
  if (value === null) {
    return null;
  }
  return text(value) ? value : undefined;
}

export function numberOrNull(value: unknown): number | null | undefined {
  if (value === null) {
    return null;
  }
  return number(value) ? value : undefined;
}

/** The items of `json[field]`, or `null` when it is not a list or any item does not read. */
export function list<T>(json: unknown, field: string, read: (item: unknown) => T | null): T[] | null {
  const items = record(json)?.[field];
  if (!Array.isArray(items)) {
    return null;
  }
  const parsed = items.map(read);
  return parsed.every((item) => item !== null) ? parsed : null;
}

/** `json[field]` read with `read`. */
export function field<T>(json: unknown, name: string, read: (value: unknown) => T | null): T | null {
  return read(record(json)?.[name]);
}

export function readUserRef(json: unknown): UserRef | null {
  const { id, display_name: displayName } = record(json) ?? {};
  return text(id) && text(displayName) ? { id, displayName } : null;
}

export function readUserRefOrNull(json: unknown): UserRef | null | undefined {
  return json === null ? null : (readUserRef(json) ?? undefined);
}

export function readRole(value: unknown): Role | null {
  return value === "owner" || value === "member" ? value : null;
}

export function readUser(json: unknown): User | null {
  const { id, username, display_name: displayName, admin, disabled, cloud } = record(json) ?? {};
  if (!text(id) || !text(username) || !text(displayName)) {
    return null;
  }
  if (typeof admin !== "boolean" || typeof disabled !== "boolean" || typeof cloud !== "boolean") {
    return null;
  }
  return { id, username, displayName, admin, disabled, cloud };
}

export function readCertificate(json: unknown): Certificate | null {
  const { check_code: checkCode, sha256, mode } = record(json) ?? {};
  if (!text(checkCode) || !text(sha256) || !/^[0-9a-f]{64}$/.test(sha256)) {
    return null;
  }
  return mode === "self_signed" || mode === "pem" ? { checkCode, sha256, mode } : null;
}

export function readCertificateOrNull(json: unknown): Certificate | null | undefined {
  return json === null ? null : (readCertificate(json) ?? undefined);
}
