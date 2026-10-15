/**
 * What a team server's admin asks of `/authorize` (`docs/architecture/server.md`, "The server admin"): a sign-in to
 * its admin (`purpose=admin`, the server's id) or linking the server to this account (`purpose=link`, the statement
 * the server signed). Either way the answer goes back by a form post to the admin's callback, so the request names
 * that callback, and a `state` the admin checks. Pure, so the rules are tested on their own.
 */

/** Where the admin takes the answer; nothing else may be asked for. */
export const CALLBACK_PATH = "/admin/auth/callback";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const STATE = /^[A-Za-z0-9_-]{16,128}$/;
const JWS = /^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/;
const MAX_STATEMENT = 16 * 1024;
const LOOPBACK = new Set(["localhost", "127.0.0.1", "[::1]"]);

export type AuthorizeRequest =
  | { readonly purpose: "admin"; readonly server: string; readonly state: string; readonly returnTo: URL }
  | { readonly purpose: "link"; readonly statement: string; readonly state: string; readonly returnTo: URL };

/** The parameters as a page or a form gives them. */
export type Params = Readonly<Record<string, string | undefined>>;

/** The callback, an absolute `https` URL (or `http` on this computer) with exactly the callback's path. */
export function callbackUrl(value: string | undefined): URL | null {
  if (value === undefined || !URL.canParse(value)) {
    return null;
  }
  const url = new URL(value);
  const secure = url.protocol === "https:" || (url.protocol === "http:" && LOOPBACK.has(url.hostname));
  const plain = url.username === "" && url.password === "" && url.search === "" && url.hash === "";
  return secure && plain && url.pathname === CALLBACK_PATH ? url : null;
}

/** A page of a team server's admin to go back to (`https://<server>/admin…`), or `null` for anything else. */
export function adminReturn(value: string): string | null {
  if (!URL.canParse(value)) {
    return null;
  }
  const url = new URL(value);
  const secure = url.protocol === "https:" || (url.protocol === "http:" && LOOPBACK.has(url.hostname));
  const admin = url.pathname === "/admin" || url.pathname.startsWith("/admin/");
  return secure && admin && url.username === "" && url.password === "" ? url.href : null;
}

/** The request, or `null` when it is not one this page answers. */
export function readRequest(params: Params): AuthorizeRequest | null {
  const state = params.state;
  const returnTo = callbackUrl(params.return);
  if (state === undefined || !STATE.test(state) || returnTo === null) {
    return null;
  }
  if (params.purpose === "admin" && params.server !== undefined && UUID.test(params.server)) {
    return { purpose: "admin", server: params.server.toLowerCase(), state, returnTo };
  }
  const statement = params.statement;
  if (params.purpose === "link" && statement !== undefined && statement.length <= MAX_STATEMENT && JWS.test(statement)) {
    return { purpose: "link", statement, state, returnTo };
  }
  return null;
}

/**
 * Where sign-in may send the visitor afterwards (`/signin?next=`): only back to `/authorize` on this site, so the
 * parameter cannot carry anyone elsewhere.
 */
export function nextAfterSignIn(value: string | undefined): string | null {
  return value !== undefined && value.startsWith("/authorize?") && !value.includes("\\") ? value : null;
}

/** The request again as query parameters, for coming back after sign-in. */
export function requestQuery(request: AuthorizeRequest): string {
  const own: [string, string] = request.purpose === "admin" ? ["server", request.server] : ["statement", request.statement];
  return new URLSearchParams([["purpose", request.purpose], ["state", request.state], ["return", request.returnTo.href], own])
    .toString();
}

/** A team server as the page shows it. */
export interface ServerView {
  readonly name: string;
  readonly address: string;
  readonly checkCode: string | null;
}

/** The answer may go back only to the server it is about: the callback is on the server's own address. */
export function returnsToServer(returnTo: URL, address: string): boolean {
  return URL.canParse(address) && new URL(address).origin === returnTo.origin;
}

/**
 * What a link statement says, for the page to show before anything is sent: **not verified** here. The cloud verifies
 * its signature when the person confirms (`POST /servers`) and refuses it then if it does not hold.
 */
export function readStatement(statement: string): ServerView | null {
  const payload = statement.split(".")[1];
  if (payload === undefined) {
    return null;
  }
  let claims: unknown;
  try {
    claims = JSON.parse(Buffer.from(payload, "base64url").toString("utf8"));
  } catch {
    return null;
  }
  const { name, address, fingerprint } = (claims ?? {}) as Record<string, unknown>;
  if (typeof name !== "string" || typeof address !== "string" || name.trim() === "") {
    return null;
  }
  const code = typeof fingerprint === "string" ? checkCode(fingerprint) : null;
  return { name: name.trim(), address, checkCode: code };
}

const CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/**
 * The check code people compare (`orchester_api::certificate::CertificatePin::check_code`): the first 80 bits of the
 * certificate's SHA-256 as 16 Crockford characters in four groups. `null` for anything that is not 64 hex characters.
 */
export function checkCode(fingerprint: string): string | null {
  if (!/^[0-9a-f]{64}$/.test(fingerprint)) {
    return null;
  }
  let bits = BigInt(`0x${fingerprint.slice(0, 20)}`);
  let text = "";
  for (let index = 0; index < 16; index += 1) {
    text = CROCKFORD.charAt(Number(bits & 31n)) + text;
    bits >>= 5n;
  }
  return [0, 4, 8, 12].map((start) => text.slice(start, start + 4)).join("-");
}

/** The address without its scheme, as the page names a server (`orchester.kontur.dev`). */
export function host(address: string): string {
  return URL.canParse(address) ? new URL(address).host : address;
}
