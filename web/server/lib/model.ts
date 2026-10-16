/**
 * What the admin's pages show, as plain data the server side reads from the team server's API and the browser
 * renders. Safe in the browser: no tokens, only what the pages print. Times are milliseconds since the epoch.
 */

export interface UserRef {
  readonly id: string;
  readonly displayName: string;
}

export interface User {
  readonly id: string;
  readonly username: string;
  readonly displayName: string;
  readonly admin: boolean;
  readonly disabled: boolean;
  /** Linked to an Orchester Cloud account: signs in with it. */
  readonly cloud: boolean;
}

/** An account on the People page, with how many projects it is in. */
export interface Person extends User {
  readonly projects: number;
}

export interface Certificate {
  readonly checkCode: string;
  /** 64 lowercase hex characters. */
  readonly sha256: string;
  readonly mode: "self_signed" | "pem";
}

export interface ServerCloud {
  /** The server's id at the cloud, for `/authorize`. */
  readonly server: string;
  readonly rootFingerprint: string | null;
  readonly accepting: boolean;
}

export interface ServerInfo {
  readonly name: string;
  readonly apiVersion: number;
  readonly needsSetup: boolean;
  readonly cloud: ServerCloud | null;
  /** `null` when the server serves plain HTTP behind a proxy that holds the certificate. */
  readonly certificate: Certificate | null;
  readonly version: string;
}

export type Role = "owner" | "member";

export interface Project {
  readonly id: string;
  readonly slug: string;
  readonly address: string;
  readonly key: string;
  readonly name: string;
  readonly remoteHint: string | null;
  /** The signed-in person's own role, `null` for an admin who is not a member. */
  readonly role: Role | null;
  readonly members: number;
  readonly openInvites: number;
  readonly owners: readonly UserRef[];
  readonly createdAt: number;
  readonly createdBy: UserRef;
}

export interface Member {
  readonly user: UserRef;
  readonly username: string;
  readonly role: Role;
  readonly addedAt: number;
}

export type InviteState = "open" | "accepted" | "expired" | "revoked";

export interface Invite {
  readonly id: string;
  readonly kind: "project" | "server" | "account";
  /** Forgotten by the server once the invite ends: ended invites have none. */
  readonly email: string | null;
  readonly project: string | null;
  readonly role: Role | null;
  readonly user: UserRef | null;
  readonly admin: boolean;
  readonly by: UserRef;
  readonly delivery: "cloud" | "code";
  readonly expiresAt: number;
  readonly createdAt: number;
  readonly state: InviteState;
  readonly endedAt: number | null;
  /** Who accepted an accepted invite, or who revoked a revoked one. */
  readonly endedBy: UserRef | null;
}

export type JoinedVia = "cloud" | "code" | "before_3";

/** A computer joined to the server (an app's device token). */
export interface Device {
  readonly id: string;
  readonly device: string;
  readonly user: { readonly id: string; readonly username: string; readonly displayName: string };
  readonly via: JoinedVia;
  readonly createdAt: number;
  readonly lastUsedAt: number;
  readonly expiresAt: number;
}

/** A browser signed in to the admin, which ends 12 hours after its sign-in. */
export interface AdminBrowser {
  readonly id: string;
  readonly user: { readonly id: string; readonly username: string; readonly displayName: string };
  readonly createdAt: number;
  readonly expiresAt: number;
  readonly current: boolean;
}

/** A computer waiting for join approval. */
export interface Approval {
  readonly id: string;
  readonly user: UserRef;
  readonly device: string;
  readonly address: string | null;
  readonly requestedAt: number;
  readonly expiresAt: number;
}

export interface Settings {
  readonly joinApproval: boolean;
  readonly name: string;
  readonly checkCode: string | null;
  readonly certificate: Certificate | null;
}

export interface CloudLink {
  readonly server: string;
  readonly rootFingerprint: string | null;
  readonly accepting: boolean;
  readonly keySetIssuedAt: number | null;
  readonly keySetExpiresAt: number | null;
  readonly linkedAt: number | null;
  readonly linkedBy: UserRef | null;
}

/** Where an email invite can go: Orchester Cloud is configured here and the server is linked to it, or why not. */
export type EmailReach =
  | { readonly kind: "ready"; readonly cloud: string }
  | { readonly kind: "no_cloud" }
  | { readonly kind: "unlinked" };
