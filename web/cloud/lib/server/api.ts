import { config } from "@/lib/server/config";

/**
 * The cloud's JSON API (`crates/api` `cloud` module, `docs/architecture/server.md` "Orchester Cloud"), called only
 * from the web front's server side: the browser never sees the API's address or a token. Every call answers an
 * `ApiResult`; failures are typed, never thrown.
 */

/** The cloud's `CloudErrorCode`, `snake_case` on the wire. */
export type CloudErrorCode =
  | "bad_request"
  | "payload_too_large"
  | "method_not_allowed"
  | "request_timeout"
  | "unsupported_media_type"
  | "not_found"
  | "unauthorized"
  | "invalid_credentials"
  | "confirm_failed"
  | "email_unconfirmed"
  | "invalid_link"
  | "invalid"
  | "rate_limited"
  | "keys_unavailable"
  | "busy"
  | "conflict"
  | "invalid_code"
  | "challenge_expired"
  | "mfa_required"
  | "session_without_mfa"
  | "internal";

export type ApiFailure =
  /** The cloud answered with its error body. `retryAfter` (seconds) comes with `rate_limited`. */
  | { readonly kind: "refused"; readonly status: number; readonly code: CloudErrorCode; readonly retryAfter?: number }
  /** No answer: the cloud is down, unreachable or too slow. */
  | { readonly kind: "unreachable" }
  /** An answer this client does not understand: a status or body the contract does not have. */
  | { readonly kind: "unexpected"; readonly status: number };

export type ApiResult<T> = { readonly ok: true; readonly value: T } | { readonly ok: false; readonly error: ApiFailure };

export interface CallOptions {
  /** The visitor's address, sent as `X-Forwarded-For` so the cloud's per-address limits see the visitor. */
  readonly client?: string | undefined;
  /** The visitor's web session token, sent as a bearer token. */
  readonly token?: string | undefined;
}

const TIMEOUT_MS = 10_000;
const CODES = new Set<string>([
  "bad_request", "payload_too_large", "method_not_allowed", "request_timeout", "unsupported_media_type", "not_found",
  "unauthorized", "invalid_credentials", "confirm_failed", "email_unconfirmed", "invalid_link", "invalid", "rate_limited",
  "keys_unavailable", "busy", "conflict", "invalid_code", "challenge_expired", "mfa_required", "session_without_mfa",
  "internal",
]);

function headers(options: CallOptions, body: boolean): Headers {
  const result = new Headers({ accept: "application/json" });
  if (body) {
    result.set("content-type", "application/json");
  }
  if (options.client !== undefined) {
    result.set("x-forwarded-for", options.client);
  }
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
  const code = (body as { error?: unknown } | null)?.error;
  if (typeof code !== "string" || !CODES.has(code)) {
    return { kind: "unexpected", status: response.status };
  }
  const seconds = retryAfter(response);
  return {
    kind: "refused",
    status: response.status,
    code: code as CloudErrorCode,
    ...(seconds === undefined ? {} : { retryAfter: seconds }),
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
    // A network error, a refused connection or the timeout: all mean the cloud did not answer.
    return null;
  }
}

/**
 * One API call. `expected` is the success status, or the success statuses when the answer can be more than one kind
 * (a sign-in: `201` signed in, `202` a two-factor challenge); `read` turns its JSON body, with its status, into the
 * value, or `null` when the body does not have the contract's shape.
 */
export async function call<T>(
  method: string,
  path: string,
  request: { body?: unknown; expected: number | readonly number[]; read: (json: unknown, status: number) => T | null },
  options: CallOptions = {},
): Promise<ApiResult<T>> {
  const response = await send(method, path, request.body, options);
  if (response === null) {
    return { ok: false, error: { kind: "unreachable" } };
  }
  const expected: readonly number[] = typeof request.expected === "number" ? [request.expected] : request.expected;
  if (!expected.includes(response.status)) {
    return { ok: false, error: await failure(response) };
  }
  // A body that is not JSON reads as `null`, which `read` refuses and which makes the answer `unexpected`.
  // A `202` may come with a body (`POST /invites` names the server) or without one (sign-up).
  const json: unknown = response.status === 204 ? null : await response.json().catch(() => null);
  const value = request.read(json, response.status);
  return value === null ? { ok: false, error: { kind: "unexpected", status: response.status } } : { ok: true, value };
}

/** For answers without a body (`202`, `204`). */
export function nothing(): true {
  return true;
}
