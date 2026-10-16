/**
 * What the Account page shows, as plain data the server loads and the browser renders (`app/account`). Safe in the
 * browser: no tokens, only what the page prints.
 */

export type Platform = "windows" | "macos" | "linux" | "web";

export interface Device {
  readonly id: string;
  readonly name: string;
  readonly platform: Platform;
  readonly lastUsedAt: number;
  /** This browser's own session. */
  readonly current: boolean;
}

/** A project invite's role; a server invite has none (the cloud is not told what the server makes the account). */
export type InviteRole = "owner" | "member";

export interface Invite {
  readonly id: string;
  readonly kind: "project" | "server";
  readonly role: InviteRole | null;
  readonly serverName: string;
  readonly serverAddress: string;
  /** `null` for an invite to the server with no project yet. */
  readonly project: { readonly name: string; readonly address: string } | null;
  readonly inviterName: string;
  readonly expiresAt: number;
  /** This browser cannot accept it yet: accepting needs two-factor authentication and a session signed in with it. */
  readonly needsTwoFactor: boolean;
}

export interface Server {
  readonly id: string;
  readonly name: string;
  readonly address: string;
  /** The account owns it (*Connected <date>*) or joined it (*Joined <date>*). */
  readonly owner: boolean;
  readonly since: number;
}

/** A section read on its own: its rows, or `failed` when the cloud did not answer, which only that section says. */
export type Listing<T> = { readonly kind: "list"; readonly items: readonly T[] } | { readonly kind: "failed" };

/** Invites are sent for 7 days, so an invite was sent 7 days before it expires (the designer's Account notes). */
export const INVITE_LIFETIME_MS = 7 * 24 * 60 * 60 * 1000;

export function appDevices(devices: readonly Device[]): number {
  return devices.filter((device) => device.platform !== "web").length;
}
