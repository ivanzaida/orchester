import { headers } from "next/headers";
import { redirect } from "next/navigation";

import type { Device, Invite, Listing, Server } from "@/lib/account";
import { devices, invites, servers } from "@/lib/server/account";
import type { Account } from "@/lib/server/cloud";
import { CloudUnavailableError } from "@/lib/server/failures";
import { account as me } from "@/lib/server/two-factor";
import { sessionEnded, signedIn } from "@/lib/server/visitor";
import type { TwoFactorStatus } from "@/lib/two-factor";

export interface AccountData {
  readonly account: Account;
  readonly twoFactor: TwoFactorStatus;
  readonly devices: readonly Device[];
  readonly invites: Listing<Invite>;
  readonly servers: Listing<Server>;
  /** The visitor's system, for which download is the button (the designer's notes). */
  readonly system: "windows" | "macos";
}

/**
 * Everything the page shows, read with the visitor's session. Without a live session it is the sign-in page; the
 * account or its devices failing is the error page; invites and servers fail on their own section only.
 */
export async function loadAccount(): Promise<AccountData> {
  const session = await signedIn();
  if (session === null) {
    redirect("/session-ended");
  }
  const [account, deviceList, inviteList, serverList] = await Promise.all([
    me(session),
    devices(session),
    invites(session),
    servers(session),
  ]);
  for (const result of [account, deviceList]) {
    if (!result.ok && sessionEnded(result.error)) {
      // A page cannot change cookies; this route forgets the session the cloud ended.
      redirect("/session-ended");
    }
  }
  if (!account.ok) {
    throw new CloudUnavailableError(account.error);
  }
  if (!deviceList.ok) {
    throw new CloudUnavailableError(deviceList.error);
  }
  const agent = (await headers()).get("user-agent") ?? "";
  const system = /Mac OS X|Macintosh/.test(agent) ? "macos" : "windows";
  const { account: found, twoFactor } = account.value;
  return { account: found, twoFactor, devices: deviceList.value, invites: inviteList, servers: serverList, system };
}
