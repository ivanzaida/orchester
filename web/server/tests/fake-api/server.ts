/**
 * A stand-in for the team server's private API (`/api/v1`) and for Orchester Cloud, on loopback, for the page tests
 * and the screenshots. The states are listed in `web/server/README.md`. Run: `node tests/fake-api/server.ts 4210 4220`.
 */
import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";

import { cloudRoute } from "./cloud.ts";
import { scenario } from "./fixtures.ts";
import type { Answer } from "./shapes.ts";
import { refuse } from "./shapes.ts";
import { adminRoute } from "./team-admin.ts";
import { authRoute, type World } from "./team-auth.ts";

interface Recorded {
  method: string;
  path: string;
  forwardedFor: string | null;
  forwardedHost: string | null;
  forwardedProto: string | null;
  authorization: string | null;
  body: Record<string, unknown> | null;
}

const world: World = { state: scenario("full", Date.now()), tokens: new Map() };
const requests: Recorded[] = [];

function send(response: ServerResponse, answer: Answer): void {
  const headers: Record<string, string> = { "content-type": "application/json" };
  if (answer.retryAfter !== undefined) {
    headers["retry-after"] = String(answer.retryAfter);
  }
  response.writeHead(answer.status, headers);
  response.end(answer.body === undefined ? undefined : JSON.stringify(answer.body));
}

async function readText(request: IncomingMessage): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of request) {
    chunks.push(chunk as Buffer);
  }
  return Buffer.concat(chunks).toString();
}

function bearer(request: IncomingMessage): string | null {
  const header = request.headers.authorization ?? "";
  return header.startsWith("Bearer ") ? header.slice(7) : null;
}

// Codes and tokens stay out of the list the tests read.
function redacted(body: Record<string, unknown>): Record<string, unknown> {
  const copy = { ...body };
  for (const key of ["code", "identity_token"]) {
    if (key in copy) {
      copy[key] = "<redacted>";
    }
  }
  return copy;
}

function route(request: IncomingMessage, text: string): Answer {
  const url = new URL(request.url ?? "/", "http://fake");
  const body = text === "" ? {} : (JSON.parse(text) as Record<string, unknown>);
  const token = bearer(request);
  const place = { host: request.headers["x-forwarded-host"]?.toString() ?? "localhost", proto: request.headers["x-forwarded-proto"]?.toString() ?? "http" };
  const call = { method: request.method ?? "", path: url.pathname, body, token, me: token === null ? null : (world.tokens.get(token) ?? null), place };
  requests.push({
    method: call.method,
    path: call.path,
    forwardedFor: request.headers["x-forwarded-for"]?.toString() ?? null,
    forwardedHost: request.headers["x-forwarded-host"]?.toString() ?? null,
    forwardedProto: request.headers["x-forwarded-proto"]?.toString() ?? null,
    authorization: request.headers.authorization ?? null,
    body: Object.keys(body).length === 0 ? null : redacted(body),
  });
  if (token !== null && call.me === null) {
    return refuse(401, "unauthorized");
  }
  return authRoute(world, call) ?? adminRoute(world, call, url.searchParams) ?? refuse(404, "not_found");
}

function fake(request: IncomingMessage, response: ServerResponse, text: string): boolean {
  const url = new URL(request.url ?? "/", "http://fake");
  if (url.pathname === "/__fake/requests") {
    send(response, { status: 200, body: requests });
    return true;
  }
  if (url.pathname === "/__fake/reset") {
    const asked = text === "" ? undefined : (JSON.parse(text) as { scenario?: unknown }).scenario;
    const name = typeof asked === "string" ? asked : "full";
    world.state = scenario(name, Date.now());
    world.tokens.clear();
    requests.length = 0;
    send(response, { status: 204 });
    return true;
  }
  return false;
}

export interface Started {
  server: Server;
  /** `http://127.0.0.1:<port>`, with the port the server got. */
  origin: string;
}

// Port 0 lets the system pick a free one, so test runs side by side never share a fake.
function listening(server: Server, port: number): Promise<Started> {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => {
      resolve({ server, origin: `http://127.0.0.1:${(server.address() as AddressInfo).port}` });
    });
  });
}

export function startFakeApi(port = 0): Promise<Started> {
  const server = createServer((request, response) => {
    void readText(request).then((text) => {
      if (!fake(request, response, text)) {
        send(response, route(request, text));
      }
    });
  });
  return listening(server, port);
}

export function startFakeCloud(port = 0): Promise<Started> {
  const server = createServer((request, response) => {
    void readText(request).then((text) => {
      if (!cloudRoute(request, response, text)) {
        send(response, refuse(404, "not_found"));
      }
    });
  });
  return listening(server, port);
}

if (process.argv[1]?.endsWith("server.ts") === true) {
  void Promise.all([startFakeApi(Number(process.argv[2] ?? "4210")), startFakeCloud(Number(process.argv[3] ?? "4220"))]).then(([api, cloud]) => {
    console.log(`fake team server API on ${api.origin}/api/v1, fake cloud on ${cloud.origin}`);
  });
}
