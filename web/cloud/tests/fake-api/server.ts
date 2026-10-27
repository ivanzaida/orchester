/**
 * A stand-in for the cloud's API (`/api/v1`) on loopback, for the page tests and for design verification. Each
 * state is reached by what is typed; `web/cloud/README.md` lists them. Run: `node tests/fake-api/server.ts 4110`.
 */
import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";

import { FULL, resetAccounts, routeSignedIn, signInAccount, signInChecked, type Answer } from "./accounts.ts";
import { routeRegistry } from "./registry.ts";
import { finishSignIn, turnOnElsewhere } from "./two-factor.ts";

import { RIGHT_PASSWORD } from "./accounts.ts";

export { RIGHT_PASSWORD };
export const ACCOUNT = { email: FULL, display_name: "Mira Koval" };

interface Recorded {
  method: string;
  path: string;
  forwardedFor: string | null;
  authorization: string | null;
  cookie: string | null;
  body: Record<string, unknown> | null;
}

const SLOW_MS = 5000;
const requests: Recorded[] = [];

const refuse = (status: number, error: string, retryAfter?: number): Answer => ({
  status,
  body: { error, message: "fake" },
  ...(retryAfter === undefined ? {} : { retryAfter }),
});

const limited = (email: unknown) => typeof email === "string" && email.startsWith("limited");
const down = (email: unknown) => typeof email === "string" && email.startsWith("down");

function emailing(email: unknown): Answer {
  if (limited(email)) {
    return refuse(429, "rate_limited", 2280);
  }
  return down(email) ? refuse(500, "internal") : { status: 202 };
}

function resend(email: unknown): Answer {
  return email === "resend-limit@example.com" ? refuse(429, "rate_limited", 2280) : emailing(email);
}

function signIn(body: Record<string, unknown>): Answer {
  const email = body.email;
  if (limited(email)) {
    return refuse(429, "rate_limited", 900);
  }
  if (email === "unconfirmed@example.com") {
    return refuse(403, "email_unconfirmed");
  }
  if (down(email)) {
    return refuse(503, "keys_unavailable");
  }
  return signInAccount(email, body.password, body.device);
}

const LINKS: Record<string, string> = {
  "confirm-ok": "confirm",
  "confirm-merged": "confirm",
  "confirm-limited": "confirm",
  "reset-ok": "reset",
};

function inspect(body: Record<string, unknown>): Answer {
  const purpose = typeof body.link_token === "string" ? LINKS[body.link_token] : undefined;
  if (body.link_token === "inspect-down") {
    return refuse(500, "internal");
  }
  return purpose === body.purpose ? { status: 200, body: { email: ACCOUNT.email } } : refuse(422, "invalid_link");
}

// Decision 65: the link and the sign-up's password together, one answer for any failure.
function confirm(body: Record<string, unknown>): Answer {
  if (body.link_token === "confirm-limited") {
    return refuse(429, "rate_limited", 3600);
  }
  // The cloud as merged answers its failure as `422 invalid_link`; row 65 says `401 confirm_failed`.
  if (body.link_token === "confirm-merged" && body.password !== RIGHT_PASSWORD) {
    return refuse(422, "invalid_link");
  }
  const live = ["confirm-ok", "confirm-merged"].includes(String(body.link_token));
  return live && body.password === RIGHT_PASSWORD ? { status: 204 } : refuse(401, "confirm_failed");
}

function finish(body: Record<string, unknown>, purpose: string): Answer {
  if (body.link_token === `${purpose}-ok`) {
    return { status: 204 };
  }
  return refuse(422, "invalid_link");
}

function bearer(request: IncomingMessage): string | null {
  const header = request.headers.authorization ?? "";
  return header.startsWith("Bearer ") ? header.slice(7) : null;
}

function route(request: IncomingMessage, path: string, body: Record<string, unknown>): Answer {
  const signedIn = routeSignedIn(request.method ?? "", path, body, bearer(request));
  if (signedIn !== undefined) {
    return signedIn;
  }
  const registry = routeRegistry(request.method ?? "", path, body);
  if (registry !== undefined) {
    return registry;
  }
  switch (`${request.method} ${path}`) {
    case "POST /api/v1/auth/signup":
      return emailing(body.email);
    case "POST /api/v1/auth/confirm/resend":
      return resend(body.email);
    case "POST /api/v1/auth/links/inspect":
      return inspect(body);
    case "POST /api/v1/auth/confirm":
      return confirm(body);
    case "POST /api/v1/auth/sessions":
      return signIn(body);
    case "POST /api/v1/auth/sessions/two-factor":
      return finishSignIn(body, signInChecked);
    case "POST /api/v1/auth/reset":
      return emailing(body.email);
    case "POST /api/v1/auth/reset/finish":
      return finish(body, "reset");
    default:
      return refuse(404, "not_found");
  }
}

function send(response: ServerResponse, answer: Answer): void {
  const headers: Record<string, string> = { "content-type": "application/json" };
  if (answer.retryAfter !== undefined) {
    headers["retry-after"] = String(answer.retryAfter);
  }
  response.writeHead(answer.status, headers);
  response.end(answer.body === undefined ? undefined : JSON.stringify(answer.body));
}

async function readBody(request: IncomingMessage): Promise<Record<string, unknown>> {
  const chunks: Buffer[] = [];
  for await (const chunk of request) {
    chunks.push(chunk as Buffer);
  }
  const text = Buffer.concat(chunks).toString();
  if (request.headers["content-type"]?.startsWith("application/x-www-form-urlencoded") === true) {
    // The answer `/authorize` posts to a team server admin's callback, which this fake also stands in for.
    return Object.fromEntries(new URLSearchParams(text));
  }
  return text === "" ? {} : (JSON.parse(text) as Record<string, unknown>);
}

// Requests are listed for the tests; passwords stay out of the list.
function redacted(body: Record<string, unknown>): Record<string, unknown> {
  const copy = { ...body };
  for (const key of ["password", "new_password", "code", "challenge"]) {
    if (key in copy) {
      copy[key] = "<redacted>";
    }
  }
  return copy;
}

export interface Started {
  server: Server;
  /** `http://127.0.0.1:<port>`, with the port the server got. */
  origin: string;
}

export function startFakeApi(port = 0): Promise<Started> {
  const server = createServer((request, response) => {
    const path = new URL(request.url ?? "/", "http://fake").pathname;
    if (path === "/__fake/requests") {
      send(response, { status: 200, body: requests });
      return;
    }
    if (path === "/__fake/reset") {
      resetAccounts();
      send(response, { status: 204 });
      return;
    }
    if (path === "/__fake/two-factor-on") {
      turnOnElsewhere(new URL(request.url ?? "/", "http://fake").searchParams.get("email") ?? "");
      send(response, { status: 204 });
      return;
    }
    void readBody(request).then((body) => {
      requests.push({
        method: request.method ?? "",
        path,
        forwardedFor: request.headers["x-forwarded-for"]?.toString() ?? null,
        authorization: request.headers.authorization ?? null,
        cookie: request.headers.cookie ?? null,
        body: Object.keys(body).length === 0 ? null : redacted(body),
      });
      // `slow…` addresses answer after 5 s, so the busy buttons (*Creating account…*) can be seen.
      const delay = typeof body.email === "string" && body.email.startsWith("slow") ? SLOW_MS : 0;
      setTimeout(() => {
        send(response, route(request, path, body));
      }, delay);
    });
  });
  // Port 0 lets the system pick a free one, so test runs side by side never share a fake.
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => {
      resolve({ server, origin: `http://127.0.0.1:${(server.address() as AddressInfo).port}` });
    });
  });
}

if (process.argv[1]?.endsWith("server.ts") === true) {
  void startFakeApi(Number(process.argv[2] ?? "4110")).then(({ origin }) => {
    console.log(`fake cloud API on ${origin}/api/v1`);
  });
}
