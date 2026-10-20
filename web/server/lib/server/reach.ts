import type { EmailReach, ServerInfo, TwoFactorStanding } from "@/lib/model";
import { config } from "@/lib/server/config";

/**
 * Whether this admin can send email invites now: it knows the cloud's address and the server is linked; and whether
 * this sign-in may, read again from the session at every page (`page`: where *Sign in again* comes back to).
 */
export function emailReach(server: ServerInfo, twoFactor: TwoFactorStanding, page: string): EmailReach {
  const cloud = config().cloudOrigin;
  if (cloud === null) {
    return { kind: "no_cloud" };
  }
  return server.cloud === null ? { kind: "unlinked" } : { kind: "ready", cloud, twoFactor, page };
}
