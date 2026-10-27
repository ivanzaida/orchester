import { readFileSync } from "node:fs";
import { createServer as createHttp, type Server } from "node:http";
import { createServer as createHttps } from "node:https";
import { createServer as createTcp, type AddressInfo, type Server as TcpServer, type Socket } from "node:net";
import { join } from "node:path";

import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

import { causeOf, deliverInvite, unreachableLine } from "@/lib/server/cloud";

import { ROOT } from "./sources";

/**
 * Why this host could not reach Orchester Cloud, from what `fetch` really throws: a refused connection, a certificate
 * it does not trust, no answer in time, or no way there; and the host's log line, which names it without the invite.
 */

function listening(server: Server | TcpServer): Promise<number> {
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      resolve((server.address() as AddressInfo).port);
    });
  });
}

function closed(server: Server | TcpServer): Promise<void> {
  return new Promise((resolve) => {
    server.close(() => {
      resolve();
    });
  });
}

async function thrown(url: string, timeoutMs = 5_000): Promise<unknown> {
  return fetch(url, { redirect: "error", signal: AbortSignal.timeout(timeoutMs) }).then(
    () => new Error("answered"),
    (error: unknown) => error,
  );
}

/** A port nothing listens on: one the system just gave out and took back. */
async function freePort(): Promise<number> {
  const server = createHttp();
  const port = await listening(server);
  await closed(server);
  return port;
}

const FIXTURES = join(ROOT, "../../crates/server/tests/fixtures");
const held: Socket[] = [];
let silent: TcpServer;
let tls: Server;
let redirecting: Server;

beforeAll(() => {
  silent = createTcp((socket) => {
    held.push(socket);
  });
  tls = createHttps(
    { cert: readFileSync(join(FIXTURES, "server.pem")), key: readFileSync(join(FIXTURES, "server-key.pem")) },
    (_, response) => {
      response.end();
    },
  );
  redirecting = createHttp((_, response) => {
    response.writeHead(302, { location: "https://elsewhere.example/" }).end();
  });
});

afterAll(async () => {
  for (const socket of held) {
    socket.destroy();
  }
  await Promise.all([closed(silent), closed(tls), closed(redirecting)]);
});

describe("the cause of a failed call to the cloud", () => {
  it("is refused when nothing listens there", async () => {
    expect(causeOf(await thrown(`http://127.0.0.1:${await freePort()}/api/v1/invites`))).toBe("refused");
  });

  it("is tls when the certificate is not one this host trusts", async () => {
    const port = await listening(tls);
    expect(causeOf(await thrown(`https://127.0.0.1:${port}/api/v1/invites`))).toBe("tls");
  });

  it("is timeout when the cloud takes the connection and never answers", async () => {
    const port = await listening(silent);
    expect(causeOf(await thrown(`http://127.0.0.1:${port}/api/v1/invites`, 200))).toBe("timeout");
  });

  it("is unreachable when the name does not resolve, there is no route, or the answer is a redirect", async () => {
    const dns = new TypeError("fetch failed", { cause: Object.assign(new Error("getaddrinfo EAI_AGAIN cloud.test"), { code: "EAI_AGAIN" }) });
    const route = new TypeError("fetch failed", { cause: Object.assign(new Error("connect EHOSTUNREACH"), { code: "EHOSTUNREACH" }) });
    expect(causeOf(dns)).toBe("unreachable");
    expect(causeOf(route)).toBe("unreachable");
    const port = await listening(redirecting);
    expect(causeOf(await thrown(`http://127.0.0.1:${port}/api/v1/invites`))).toBe("unreachable");
  });

  it("looks inside the errors of every address a connection tried", () => {
    const refused = Object.assign(new Error("connect ECONNREFUSED ::1:443"), { code: "ECONNREFUSED" });
    const both = new TypeError("fetch failed", { cause: new AggregateError([refused, refused]) });
    expect(causeOf(both)).toBe("refused");
  });

  it("is said in the host's log with its code and words", () => {
    const dns = new TypeError("fetch failed", { cause: Object.assign(new Error("getaddrinfo ENOTFOUND cloud.test"), { code: "ENOTFOUND" }) });
    expect(unreachableLine("https://cloud.test", dns)).toBe(
      "orchester-admin: cannot reach Orchester Cloud at https://cloud.test: unreachable (ENOTFOUND: getaddrinfo ENOTFOUND cloud.test)",
    );
  });
});

describe("delivering an invite when this host cannot reach the cloud", () => {
  it("answers unreachable with the cause, and logs it without the signed invite", async () => {
    const origin = `http://127.0.0.1:${await freePort()}`;
    process.env.ORCHESTER_ADMIN_SERVER_URL = `http://127.0.0.1:${await freePort()}`;
    process.env.ORCHESTER_ADMIN_SECRET = Buffer.alloc(32, 9).toString("base64");
    process.env.ORCHESTER_ADMIN_CLOUD_URL = origin;
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const delivery = await deliverInvite("signed-invite-for-dana");
    expect(delivery).toEqual({ kind: "unreachable", cause: "refused" });
    const lines = warn.mock.calls.map((call) => String(call[0]));
    expect(lines).toHaveLength(1);
    expect(lines[0]).toContain(`cannot reach Orchester Cloud at ${origin}: refused (ECONNREFUSED`);
    expect(lines.join("\n")).not.toContain("signed-invite-for-dana");
    warn.mockRestore();
  });
});
