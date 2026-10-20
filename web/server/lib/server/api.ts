import { config } from "@/lib/server/config";
import type { Place } from "@/lib/server/place";

/**
 * The team server's JSON API (`/api/v1`, `docs/architecture/server.md` "API"), called only from the admin's server
 * side over the server's private listener: the browser never sees the API's address or a token. Every call answers
 * an `ApiResult`; failures are typed, never thrown.
 */

/** The error codes the admin meets (contract, "Server API"), `snake_case` on the wire. */
export type ServerErrorCode =
  | "bad_request"
  | "unauthorized"
  | "forbidden"
  | "not_found"
  | "invalid"
  | "username_taken"
  | "last_admin"
  | "last_owner"
  | "no_such_user"
  | "invalid_identity"
  | "not_invited"
  | "cloud_off"
  | "key_set_stale"
  | "invalid_code"
  | "too_many_codes"
  | "slug_taken"
  | "nothing_to_manage"
  | "approval_required"
  | "mfa_required"
  | "invite_open"
  | "already_member"
  | "rate_limited"
  | "conflict"
  | "internal";

const CODES = new Set<string>([
  "bad_request", "unauthorized", "forbidden", "not_found", "invalid", "username_taken", "last_admin", "last_owner",
  "no_such_user", "invalid_identity", "not_invited", "cloud_off", "key_set_stale", "invalid_code", "too_many_codes",
  "slug_taken", "nothing_to_manage", "approval_required", "mfa_required", "invite_open", "already_member",
  "rate_limited", "conflict", "internal",
]);

export type ApiFailure =
  /**
   * The server answered with its error body. `retryAfter` (seconds) comes with `rate_limited`; `sentAt` (milliseconds)
   * with `invite_open`: when the open invite it names was sent.
   */
  | {
      readonly kind: "refused";
      readonly status: number;
      readonly code: ServerErrorCode;
      readonly retryAfter?: number;
      readonly sentAt?: number;
    }
  /** No answer: the server is down, unreachable or too slow. */
  | { readonly kind: "unreachable" }
  /** An answer this client does not understand: a status or body the contract does not have. */
  | { readonly kind: "unexpected"; readonly status: number };

export type ApiResult<T> = { readonly ok: true; readonly value: T } | { readonly ok: false; readonly error: ApiFailure };

/** Who the call is for: the person's admin web token, and where the browser is, which the server needs to know. */
export interface CallOptions {
  readonly token?: string | undefined;
  /** The client's address, passed on as `X-Forwarded-For` so the server's per-address limits see the person. */
  readonly client?: string | undefined;
  /** The browser's view of the server, passed on as `X-Forwarded-Host`/`-Proto`: project addresses are made from it. */
  readonly place: Place;
}

const TIMEOUT_MS = 10_000;

function headers(options: CallOptions, body: boolean): Headers {
  const result = new Headers({ accept: "application/json" });
  if (body) {
    result.set("content-type", "application/json");
  }
  if (options.client !== undefined) {
    result.set("x-forwarded-for", options.client);
  }
  result.set("x-forwarded-host", options.place.host);
  result.set("x-forwarded-proto", options.place.proto);
  if (options.token !== undefined) {
    result.set("authorization", `Bearer ${options.token}`);
  }
  return result;
}

function retryAfter(response: Response): number | undefined {
  const seconds = Number(response.headers.get("retry-after"));
  return Number.isFinite(seconds) && seconds > 0 ? seconds : undefined;
}

async function failure(response: Response): Promise<ApiFailure> {
  // A body that is not JSON has no error code, which makes the answer `unexpected` below.
  const body: unknown = await response.json().catch(() => null);
  const { error: code, sent_at: sentAt } = (body ?? {}) as { error?: unknown; sent_at?: unknown };
  if (typeof code !== "string" || !CODES.has(code)) {
    return { kind: "unexpected", status: response.status };
  }
  const seconds = retryAfter(response);
  return {
    kind: "refused",
    status: response.status,
    code: code as ServerErrorCode,
    ...(seconds === undefined ? {} : { retryAfter: seconds }),
    ...(typeof sentAt === "number" && Number.isFinite(sentAt) ? { sentAt } : {}),
  };
}

async function send(method: string, path: string, body: unknown, options: CallOptions): Promise<Response | null> {
  try {
    return await fetch(`${config().apiBase}${path}`, {
      method,
      headers: headers(options, body !== undefined),
      body: body === undefined ? null : JSON.stringify(body),
      cache: "no-store",
      redirect: "error",
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
  } catch {
    // A network error, a refused connection or the timeout: all mean the server did not answer.
    return null;
  }
}

export interface Request<T> {
  readonly body?: unknown;
  readonly expected: number;
  /** Turns the JSON body into the value, or `null` when it does not have the contract's shape. */
  readonly read: (json: unknown) => T | null;
}

/** One API call; `expected` is the success status. */
export async function call<T>(method: string, path: string, request: Request<T>, options: CallOptions): Promise<ApiResult<T>> {
  const response = await send(method, path, request.body, options);
  if (response === null) {
    return { ok: false, error: { kind: "unreachable" } };
  }
  if (response.status !== request.expected) {
    return { ok: false, error: await failure(response) };
  }
  // A body that is not JSON reads as `null`, which `read` refuses and which makes the answer `unexpected`.
  const json: unknown = response.status === 204 ? null : await response.json().catch(() => null);
  const value = request.read(json);
  return value === null ? { ok: false, error: { kind: "unexpected", status: response.status } } : { ok: true, value };
}

/** For answers without a body (`204`). */
export function nothing(): true {
  return true;
}

/** Whether a failure is this refusal. */
export function refused(failure: ApiFailure, ...codes: ServerErrorCode[]): boolean {
  return failure.kind === "refused" && codes.includes(failure.code);
}

/**
 * Ids come from a posted form or a page address, so they are checked before they go into an API path: the server's
 * ids are UUIDs. Anything else is refused here as the server would refuse a malformed request, and no request is made.
 */
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export const NOT_AN_ID: ApiResult<never> = { ok: false, error: { kind: "refused", status: 400, code: "bad_request" } };

export function isId(value: string): boolean {
  return UUID.test(value);
}
