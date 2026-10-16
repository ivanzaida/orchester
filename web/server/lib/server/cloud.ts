import { config } from "@/lib/server/config";

/**
 * The admin's own calls to Orchester Cloud (contract, "Orchester Cloud, as the admin and /authorize use it"):
 * delivering a server-signed invite or revocation. The signature inside is what the cloud checks; no token is sent.
 * The team server needs no internet: when this host cannot reach the cloud either, the page offers to send the same
 * signed invite from the person's browser.
 */
export type Delivery = "delivered" | "unreachable" | "refused" | "no_cloud";

const TIMEOUT_MS = 8_000;

async function post(method: "POST" | "DELETE", path: string, body: unknown, expected: number): Promise<Delivery> {
  const origin = config().cloudOrigin;
  if (origin === null) {
    return "no_cloud";
  }
  try {
    const response = await fetch(`${origin}/api/v1${path}`, {
      method,
      headers: { accept: "application/json", "content-type": "application/json" },
      body: JSON.stringify(body),
      cache: "no-store",
      redirect: "error",
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
    return response.status === expected ? "delivered" : "refused";
  } catch {
    // A network error, a refused connection or the timeout: this host cannot reach the cloud now.
    return "unreachable";
  }
}

export function deliverInvite(signed: string): Promise<Delivery> {
  return post("POST", "/invites", { invite: signed }, 202);
}

export function deliverRevocation(inviteId: string, revocation: string): Promise<Delivery> {
  return post("DELETE", `/invites/${encodeURIComponent(inviteId)}`, { revocation }, 204);
}
