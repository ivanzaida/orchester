/**
 * The fake cloud's team server registry, and a team server admin's callback that `/authorize` posts its answer to:
 * the fake API's own address stands in for the server (`http://127.0.0.1:4110`, loopback, so plain HTTP is allowed).
 */
import type { Answer } from "./accounts.ts";

export const SERVER_ID = "0199f0a0-0000-7000-8000-00000000c10d";
export const SERVER_ADDRESS = "http://127.0.0.1:4110";
const FINGERPRINT = "ab".repeat(32);

const server = { id: SERVER_ID, name: "Kontur team", address: SERVER_ADDRESS, fingerprint: FINGERPRINT,
  check_code: "NENT-QAXB-NENT-QAXB" };

export function routeRegistry(method: string, path: string, body: Record<string, unknown>): Answer | undefined {
  switch (`${method} ${path}`) {
    case `GET /api/v1/servers/${SERVER_ID}`:
      return { status: 200, body: { server } };
    case "POST /api/v1/servers":
      return typeof body.link === "string" && body.link.split(".").length === 3
        ? { status: 201, body: { server, root_key: "fake-root-key", key_set: "fake-key-set", identity_token: "fake-link-token",
          expires_at: 1 } }
        : { status: 422, body: { error: "invalid", message: "fake" } };
    case "POST /api/v1/identity-tokens":
      return body.audience === SERVER_ID
        ? { status: 201, body: { identity_token: "fake-admin-token", expires_at: 1 } }
        : { status: 404, body: { error: "not_found", message: "fake" } };
    case "GET /api/v1/keys":
      return { status: 200, body: { key_set: "fake-key-set" } };
    case "POST /api/v1/invites":
      return { status: 202, body: { server: { id: SERVER_ID, name: server.name, address: SERVER_ADDRESS } } };
    case "POST /admin/auth/callback":
      return { status: 200, body: { received: true } };
    default:
      return path.startsWith("/api/v1/servers/") ? { status: 404, body: { error: "not_found", message: "fake" } } : undefined;
  }
}
