import type { EmailReach, ServerInfo } from "@/lib/model";
import { config } from "@/lib/server/config";

/** Whether this admin can send email invites now: it knows the cloud's address and the server is linked. */
export function emailReach(server: ServerInfo): EmailReach {
  const cloud = config().cloudOrigin;
  if (cloud === null) {
    return { kind: "no_cloud" };
  }
  return server.cloud === null ? { kind: "unlinked" } : { kind: "ready", cloud };
}
