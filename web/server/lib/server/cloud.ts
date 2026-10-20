import { config } from "@/lib/server/config";
import type { CloudCause } from "@/lib/model";

/**
 * The admin's own calls to Orchester Cloud (contract, "Orchester Cloud, as the admin and /authorize use it"):
 * delivering a server-signed invite or revocation. The signature inside is what the cloud checks; no token is sent.
 * The team server needs no internet: when this host cannot reach the cloud either, the page offers to send the same
 * signed invite from the person's browser, and says why this host could not.
 */
export type Delivery =
  | { readonly kind: "delivered" }
  /** The cloud answered, with another status than the contract's. */
  | { readonly kind: "declined" }
  | { readonly kind: "no_cloud" }
  | { readonly kind: "unreachable"; readonly cause: CloudCause };

const TIMEOUT_MS = 8_000;

/**
 * Node's error codes for a certificate this host does not trust (OpenSSL's verify results, e.g.
 * `UNABLE_TO_VERIFY_LEAF_SIGNATURE`, `DEPTH_ZERO_SELF_SIGNED_CERT`, `CERT_HAS_EXPIRED`), or a TLS handshake that failed.
 */
const TLS = /CERT|SIGNATURE|^UNABLE_TO_|^ERR_TLS_|^ERR_SSL_|^EPROTO$/;
const TIMEOUTS = new Set(["ETIMEDOUT", "UND_ERR_CONNECT_TIMEOUT", "UND_ERR_HEADERS_TIMEOUT", "UND_ERR_BODY_TIMEOUT"]);

interface Thrown {
  readonly name: string;
  readonly code: string | null;
  readonly message: string;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/**
 * What a failed `fetch` threw, as the error and the causes inside it: `fetch failed` wraps the real one, and a
 * connection tried on several addresses wraps one per address.
 */
function chain(error: unknown): Thrown[] {
  const found: Thrown[] = [];
  const queue = [error].filter(isObject);
  for (let next = queue.shift(); next !== undefined && found.length < 8; next = queue.shift()) {
    const { name, code, message, cause, errors } = next;
    found.push({
      name: typeof name === "string" ? name : "",
      code: typeof code === "string" ? code : null,
      message: typeof message === "string" ? message : "",
    });
    queue.push(...[cause, ...(Array.isArray(errors) ? (errors as unknown[]) : [])].filter(isObject));
  }
  return found;
}

/** Why a `fetch` to the cloud threw: a certificate, a refused connection, no answer in time, or no way there at all. */
export function causeOf(error: unknown): CloudCause {
  const links = chain(error);
  const codes = links.flatMap((link) => (link.code === null ? [] : [link.code]));
  if (codes.some((code) => TLS.test(code))) {
    return "tls";
  }
  if (codes.includes("ECONNREFUSED")) {
    return "refused";
  }
  if (links.some((link) => link.name === "TimeoutError") || codes.some((code) => TIMEOUTS.has(code))) {
    return "timeout";
  }
  return "unreachable";
}

/**
 * The host's log line for the person who runs the admin: the cause, with the error's own code and words (a host name,
 * an address, a certificate problem). The request carries no token and no key, and its body is never logged.
 */
export function unreachableLine(origin: string, error: unknown): string {
  const links = chain(error);
  const inner = links[links.length - 1];
  const code = links.find((link) => link.code !== null)?.code ?? inner?.name ?? "";
  const words = (inner?.message ?? "").slice(0, 200);
  return `orchester-admin: cannot reach Orchester Cloud at ${origin}: ${causeOf(error)} (${code}: ${words})`;
}

async function post(method: "POST" | "DELETE", path: string, body: unknown, expected: number): Promise<Delivery> {
  const origin = config().cloudOrigin;
  if (origin === null) {
    return { kind: "no_cloud" };
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
    return response.status === expected ? { kind: "delivered" } : { kind: "declined" };
  } catch (error) {
    console.warn(unreachableLine(origin, error));
    return { kind: "unreachable", cause: causeOf(error) };
  }
}

export function deliverInvite(signed: string): Promise<Delivery> {
  return post("POST", "/invites", { invite: signed }, 202);
}

export function deliverRevocation(inviteId: string, revocation: string): Promise<Delivery> {
  return post("DELETE", `/invites/${encodeURIComponent(inviteId)}`, { revocation }, 204);
}
