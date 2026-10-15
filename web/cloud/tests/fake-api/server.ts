/**
 * A stand-in for the cloud's API (`/api/v1`) on loopback, for the page tests and for design verification. Each
 * state is reached by what is typed; `web/cloud/README.md` lists them. Run: `node tests/fake-api/server.ts 4110`.
 */
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";

export const ACCOUNT = { email: "mira.koval@example.com", display_name: "Mira Koval" };
export const RIGHT_PASSWORD = "correct horse battery";

interface Recorded {
  method: string;
  path: string;
  forwardedFor: string | null;
  authorization: string | null;
  cookie: string | null;
  body: Record<string, unknown> | null;
}

interface Answer {
  status: number;
  body?: unknown;
  retryAfter?: number;
}

const SLOW_MS = 5000;
const requests: Recorded[] = [];
const sessions = new Set<string>();
let issued = 0;

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
  if (email !== ACCOUNT.email || body.password !== RIGHT_PASSWORD) {
    return refuse(401, "invalid_credentials");
  }
  issued += 1;
  const token = `ocd_fake_web_token_${issued}`;
  sessions.add(token);
  const device = { id: `device-${issued}`, name: "fake", platform: "web", created_at: 0, last_used_at: 0, current: true };
  return { status: 201, body: { token, device, account: { id: "account-1", ...ACCOUNT, created_at: 0 } } };
}

const LINKS: Record<string, string> = { "confirm-ok": "confirm", "reset-ok": "reset" };

function inspect(body: Record<string, unknown>): Answer {
  const purpose = typeof body.link_token === "string" ? LINKS[body.link_token] : undefined;
  if (body.link_token === "inspect-down") {
    return refuse(500, "internal");
  }
  return purpose === body.purpose ? { status: 200, body: { email: ACCOUNT.email } } : refuse(422, "invalid_link");
}

function finish(body: Record<string, unknown>, purpose: string): Answer {
  if (body.link_token === `${purpose}-ok`) {
    return { status: 204 };
  }
  return refuse(422, "invalid_link");
}

function bearer(request: IncomingMessage): string | null {
  const header = request.headers.authorization ?? "";
  const token = header.startsWith("Bearer ") ? header.slice(7) : null;
  return token !== null && sessions.has(token) ? token : null;
}

function route(request: IncomingMessage, path: string, body: Record<string, unknown>): Answer {
  switch (`${request.method} ${path}`) {
    case "POST /api/v1/auth/signup":
      return emailing(body.email);
    case "POST /api/v1/auth/confirm/resend":
      return resend(body.email);
    case "POST /api/v1/auth/links/inspect":
      return inspect(body);
    case "POST /api/v1/auth/confirm":
      return finish(body, "confirm");
    case "POST /api/v1/auth/sessions":
      return signIn(body);
    case "POST /api/v1/auth/reset":
      return emailing(body.email);
    case "POST /api/v1/auth/reset/finish":
      return finish(body, "reset");
    case "GET /api/v1/me":
      return bearer(request) === null ? refuse(401, "unauthorized") : { status: 200, body: { account: { id: "account-1", ...ACCOUNT, created_at: 0 } } };
    case "DELETE /api/v1/auth/sessions/current": {
      const token = bearer(request);
      return token !== null && sessions.delete(token) ? { status: 204 } : refuse(401, "unauthorized");
    }
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
  return text === "" ? {} : (JSON.parse(text) as Record<string, unknown>);
}

// Requests are listed for the tests; passwords stay out of the list.
function redacted(body: Record<string, unknown>): Record<string, unknown> {
  const copy = { ...body };
  for (const key of ["password", "new_password"]) {
    if (key in copy) {
      copy[key] = "<redacted>";
    }
  }
  return copy;
}

export function startFakeApi(port: number) {
  const server = createServer((request, response) => {
    const path = new URL(request.url ?? "/", "http://fake").pathname;
    if (path === "/__fake/requests") {
      send(response, { status: 200, body: requests });
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
  server.listen(port, "127.0.0.1");
  return server;
}

if (process.argv[1]?.endsWith("server.ts") === true) {
  const port = Number(process.argv[2] ?? "4110");
  startFakeApi(port);
  console.log(`fake cloud API on http://127.0.0.1:${port}/api/v1`);
}
