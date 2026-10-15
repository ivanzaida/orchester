/**
 * Where the browser reached the admin. The team server is the front door and reverse-proxies the admin; it
 * overwrites `X-Forwarded-Proto` and `X-Forwarded-Host`, and the admin listens only on a private address, so it trusts
 * them. Without them (development, page tests) the `Host` header and plain `http` stand in. No `node:` imports:
 * `proxy.ts` uses this too.
 */
export interface Place {
  readonly proto: "http" | "https";
  /** The `Host` the browser used, with its port. */
  readonly host: string;
}

interface HeaderSource {
  get(name: string): string | null;
}

const LOOPBACK = new Set(["localhost", "127.0.0.1", "[::1]"]);
const HOST = /^[A-Za-z0-9.-]+(?::\d{1,5})?$|^\[[0-9A-Fa-f:.]+\](?::\d{1,5})?$/;

function first(value: string | null): string | null {
  const head = value?.split(",")[0]?.trim();
  return head === undefined || head === "" ? null : head;
}

export function placeOf(headers: HeaderSource): Place {
  const proto = first(headers.get("x-forwarded-proto")) === "https" ? "https" : "http";
  const host = first(headers.get("x-forwarded-host")) ?? first(headers.get("host")) ?? "localhost";
  // A host the browser could not have sent is refused as the loopback name, which never matches a form's Origin.
  return { proto, host: HOST.test(host) ? host.toLowerCase() : "localhost" };
}

export function originOf(place: Place): string {
  return `${place.proto}://${place.host}`;
}

export function hostname(place: Place): string {
  return new URL(originOf(place)).hostname;
}

/** Plain HTTP is refused except on the server itself: the admin's cookies are `Secure`. */
export function needsHttps(place: Place): boolean {
  return place.proto === "http" && !LOOPBACK.has(hostname(place));
}

/** The client's one address, which the server's proxy put in `X-Forwarded-For`; none in development. */
export function clientOf(headers: HeaderSource): string | undefined {
  return first(headers.get("x-forwarded-for")) ?? undefined;
}
