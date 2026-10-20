import type { TwoFactorStanding } from "@/lib/model";
import { HANDOFF_SECONDS, NOTHING_SECONDS, SESSION_SECONDS, SETUP_SECONDS, STATE_SECONDS } from "@/lib/server/cookie-options";
import { open, sameText, seal, sign, verify } from "@/lib/server/seal";

/**
 * What the admin's cookies hold. Each value carries its own expiry, so one kept or replayed past it is refused here
 * even if a browser still sends it. Tokens and codes are sealed (AES-256-GCM): the browser keeps an opaque value it
 * can neither read nor use against the API.
 */

/** How the person signed in, for the header's *signed in with Orchester Cloud* (`NNrob`). */
export type SignedInWith = "cloud" | "code";

export interface AdminSession {
  readonly token: string;
  readonly via: SignedInWith;
  /** What the server said of this sign-in's two-factor authentication when it signed in; it never changes after. */
  readonly twoFactor: TwoFactorStanding;
}

const STANDINGS = new Set<unknown>(["on", "without", "off", "unknown"]);

/**
 * What a new sign-in's session says (`session.mfa` of the server's answer; `null` from a server before it). `again`:
 * the sign-in was *Sign in again* through Orchester Cloud, so one without it means the account has none to use now.
 */
export function signedInStanding(mfa: boolean | null, again: boolean): TwoFactorStanding {
  if (mfa === null) {
    return "unknown";
  }
  if (mfa) {
    return "on";
  }
  return again ? "off" : "without";
}

function standing(value: unknown): TwoFactorStanding {
  return STANDINGS.has(value) ? (value as TwoFactorStanding) : "unknown";
}

/**
 * The admin's own pages *Sign in again* may come back to: a path under `/admin`, of plain segments, nothing else (no
 * other origin, no `//`, no `..`, no query).
 */
export function isAdminPage(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length <= 200 &&
    /^\/admin(\/[A-Za-z0-9._~-]+)*$/.test(value) &&
    !value.split("/").some((segment) => segment === "." || segment === "..")
  );
}

function parse(text: string | null): Record<string, unknown> | null {
  if (text === null) {
    return null;
  }
  try {
    const value: unknown = JSON.parse(text);
    return typeof value === "object" && value !== null ? (value as Record<string, unknown>) : null;
  } catch {
    // Only a value this admin made reaches here; anything else is no cookie at all.
    return null;
  }
}

function live(value: Record<string, unknown> | null, now: number): Record<string, unknown> | null {
  return value !== null && typeof value.expires === "number" && value.expires > now ? value : null;
}

function sealed(key: Buffer, value: string | undefined, now: number): Record<string, unknown> | null {
  return live(parse(value === undefined ? null : open(key, value)), now);
}

function signed(key: Buffer, value: string | undefined, now: number): Record<string, unknown> | null {
  return live(parse(value === undefined ? null : verify(key, value)), now);
}

export function sealSession(key: Buffer, session: AdminSession, now: number): string {
  return seal(key, JSON.stringify({ ...session, expires: now + SESSION_SECONDS * 1000 }));
}

/** A session as a cookie holds it; one from before the admin kept its two-factor standing reads as `unknown`. */
function readSession(value: unknown): AdminSession | null {
  const { token, via, twoFactor } = (typeof value === "object" && value !== null ? value : {}) as Record<string, unknown>;
  if (typeof token !== "string" || (via !== "cloud" && via !== "code")) {
    return null;
  }
  return { token, via, twoFactor: standing(twoFactor) };
}

export function openSession(key: Buffer, value: string | undefined, now: number): AdminSession | null {
  return readSession(sealed(key, value, now));
}

/** The setup code between setup steps 1 and 2 (15 minutes). It is never put in a page, a URL or a log. */
export function sealSetup(key: Buffer, code: string, now: number): string {
  return seal(key, JSON.stringify({ code, expires: now + SETUP_SECONDS * 1000 }));
}

export function openSetup(key: Buffer, value: string | undefined, now: number): string | null {
  const setup = sealed(key, value, now);
  return typeof setup?.code === "string" ? setup.code : null;
}

/** A trip to the cloud's `/authorize`: what for, and where the admin goes back to after a link. */
export type TripPurpose = "admin" | "link";
export type TripBack = "signin" | "setup" | "settings";

/**
 * The trip's cookie is sealed, not only signed: a link trip carries the admin's session token, because the cloud
 * comes back with a cross-site form `POST`, which the `SameSite=Lax` session cookie does not follow.
 */
export interface Trip {
  readonly state: string;
  readonly purpose: TripPurpose;
  readonly back: TripBack;
  /** The admin session that started a link, to finish it with. */
  readonly session: AdminSession | null;
  /** *Sign in again*: the admin page it started on and comes back to; `null` for any other trip. */
  readonly page: string | null;
}

const PURPOSES = new Set<unknown>(["admin", "link"]);
const BACKS = new Set<unknown>(["signin", "setup", "settings"]);

export function newState(): string {
  return Buffer.from(crypto.getRandomValues(new Uint8Array(32))).toString("base64url");
}

export function sealTrip(key: Buffer, trip: Trip, now: number): string {
  return seal(key, JSON.stringify({ ...trip, expires: now + STATE_SECONDS * 1000 }));
}

function tripSession(value: unknown): AdminSession | null | undefined {
  return value === null ? null : (readSession(value) ?? undefined);
}

export function openTrip(key: Buffer, value: string | undefined, now: number): Trip | null {
  const trip = sealed(key, value, now);
  const session = tripSession(trip?.session);
  if (typeof trip?.state !== "string" || !PURPOSES.has(trip.purpose) || !BACKS.has(trip.back) || session === undefined) {
    return null;
  }
  // A trip cookie from before *Sign in again* has no page: it is any other trip.
  const page = isAdminPage(trip.page) ? trip.page : null;
  return { state: trip.state, purpose: trip.purpose as TripPurpose, back: trip.back as TripBack, session, page };
}

/**
 * The callback's own CSRF check: the posted `state` is the one this browser's cookie carries, compared in constant
 * time. The cloud's form comes from another origin, so the usual `Origin` check cannot apply.
 */
export function tripMatches(trip: Trip, submitted: unknown): boolean {
  return typeof submitted === "string" && sameText(trip.state, submitted);
}

/** A trip's form to the cloud: where it goes and its fields. */
export interface Handoff {
  readonly action: string;
  readonly fields: readonly (readonly [string, string])[];
}

export function sealHandoff(key: Buffer, handoff: Handoff, now: number): string {
  return seal(key, JSON.stringify({ ...handoff, expires: now + HANDOFF_SECONDS * 1000 }));
}

function pairs(value: unknown): [string, string][] | null {
  if (!Array.isArray(value)) {
    return null;
  }
  const fields = value.filter(
    (item): item is [string, string] => Array.isArray(item) && item.length === 2 && item.every((part) => typeof part === "string"),
  );
  return fields.length === value.length ? fields : null;
}

export function openHandoff(key: Buffer, value: string | undefined, now: number): Handoff | null {
  const handoff = sealed(key, value, now);
  const fields = pairs(handoff?.fields);
  return typeof handoff?.action === "string" && fields !== null ? { action: handoff.action, fields } : null;
}

/** Whom the cloud named when the account has nothing to manage here; shown on that page only. */
export interface NothingFlow {
  readonly name: string | null;
  readonly email: string | null;
}

export function signNothing(key: Buffer, flow: NothingFlow, now: number): string {
  return sign(key, JSON.stringify({ ...flow, expires: now + NOTHING_SECONDS * 1000 }));
}

export function readNothing(key: Buffer, value: string | undefined, now: number): NothingFlow | null {
  const flow = signed(key, value, now);
  if (flow === null) {
    return null;
  }
  const text = (field: unknown) => (typeof field === "string" ? field : null);
  return { name: text(flow.name), email: text(flow.email) };
}
