/**
 * A stand-in for Orchester Cloud as the admin meets it: `/authorize` (a plain page that posts back to the admin's
 * callback, one button per test identity), `POST/DELETE /api/v1/invites` (delivery from the admin's server side) and
 * `/invites/deliver` (delivery through the browser). Not the cloud's real pages: those are web/cloud's.
 */
import type { IncomingMessage, ServerResponse } from "node:http";

import type { Answer } from "./shapes.ts";

export interface Delivery {
  readonly how: "server" | "browser" | "revocation";
  readonly email: string;
}

export const deliveries: Delivery[] = [];

const IDENTITIES: readonly [email: string, name: string][] = [
  ["ivan@example.com", "Ivan Zaida"],
  ["maria@example.com", "Maria Lind"],
  ["dana@example.com", "Dana Ruiz"],
  ["stranger@example.com", "Sam Stranger"],
  ["stale@example.com", "Stale Keys"],
  ["approval@example.com", "Approval Needed"],
];

function escape(text: string): string {
  return text.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

/** A fake identity token: `header.payload.signature`, the payload readable like the cloud's. */
export function identityToken(email: string, name: string): string {
  const part = (value: unknown) => Buffer.from(JSON.stringify(value)).toString("base64url");
  return `${part({ alg: "EdDSA" })}.${part({ email, name, aud: "fake" })}.fake-signature`;
}

function hidden(name: string, value: string): string {
  return `<input type="hidden" name="${escape(name)}" value="${escape(value)}">`;
}

/** What the trip posted: the fake shows it, so a test can see the statement came in the body. */
export const posted: Record<string, string>[] = [];

function authorize(fields: Record<string, string>): string {
  const purpose = fields.purpose ?? "";
  const back = fields.return ?? "";
  const state = fields.state ?? "";
  const forms = IDENTITIES.map(([email, name]) => {
    const fields = [hidden("state", state), hidden("identity_token", identityToken(email, name)), hidden("key_set", "fake-key-set")];
    if (purpose === "link") {
      fields.push(hidden("root_key", "fake-root-key"));
    }
    return `<form method="post" action="${escape(back)}">${fields.join("")}<button>Continue as ${escape(email)}</button></form>`;
  });
  const cancel = `<a href="${escape(new URL("/admin", back || "http://localhost").toString())}">Cancel</a>`;
  return `<!doctype html><title>Fake Orchester Cloud</title><h1>Fake cloud: ${escape(purpose)}</h1>${forms.join("")}${cancel}`;
}

function email(signed: unknown): string {
  const part = typeof signed === "string" ? signed.split(".")[1] : undefined;
  return part === undefined ? "" : Buffer.from(part, "base64url").toString();
}

/** The cloud's API for invites. `offline…` addresses: no answer at all, as when the admin's host has no internet. */
function invites(method: string, body: Record<string, unknown>): Answer | "drop" {
  const address = email(method === "POST" ? body.invite : body.revocation);
  if (address.startsWith("offline")) {
    return "drop";
  }
  if (address.startsWith("refused")) {
    return { status: 422, body: { error: "invalid", message: "fake" } };
  }
  deliveries.push({ how: method === "POST" ? "server" : "revocation", email: address });
  return { status: method === "POST" ? 202 : 204 };
}

function form(text: string): Record<string, string> {
  return Object.fromEntries(new URLSearchParams(text));
}

/** Answers the fake cloud's paths; `false` when the path is not one of them. */
export function cloudRoute(request: IncomingMessage, response: ServerResponse, text: string): boolean {
  const url = new URL(request.url ?? "/", "http://fake-cloud");
  const html = (status: number, body: string) => {
    response.writeHead(status, { "content-type": "text/html; charset=utf-8" });
    response.end(body);
  };
  if (url.pathname === "/authorize/start" && request.method === "POST") {
    const fields = form(text);
    posted.push(fields);
    html(200, authorize(fields));
    return true;
  }
  if (url.pathname === "/invites/deliver" && request.method === "POST") {
    const fields = form(text);
    deliveries.push({ how: "browser", email: email(fields.invite) });
    html(200, `<!doctype html><title>Delivered</title><p>Invite delivered.</p><a href="${escape(fields.return ?? "")}">Back</a>`);
    return true;
  }
  if (/^\/api\/v1\/invites(\/[^/]+)?$/.test(url.pathname)) {
    const answer = invites(request.method ?? "", text === "" ? {} : (JSON.parse(text) as Record<string, unknown>));
    if (answer === "drop") {
      request.socket.destroy();
      return true;
    }
    response.writeHead(answer.status, { "content-type": "application/json" });
    response.end(answer.body === undefined ? undefined : JSON.stringify(answer.body));
    return true;
  }
  if (url.pathname === "/__fake/deliveries") {
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify(deliveries));
    return true;
  }
  if (url.pathname === "/__fake/authorize") {
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify(posted));
    return true;
  }
  return false;
}
