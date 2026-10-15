/**
 * The admin's configuration, from the environment only (`docs/architecture/server.md`, "The server admin"): the
 * team server's private API listener, the secret that signs and seals the cookies, and optionally Orchester Cloud's
 * public address. The admin's own public address is not configured: the server's proxy names it per request.
 */
export interface Config {
  /** The team server's API base, `<ORCHESTER_ADMIN_SERVER_URL>/api/v1`, without a trailing slash. */
  readonly apiBase: string;
  readonly secret: Uint8Array;
  /** Orchester Cloud's origin, or `null` when the admin has none: cloud sign-in, linking and email invites are off. */
  readonly cloudOrigin: string | null;
}

export class ConfigError extends Error {
  constructor(readonly variable: string, reason: string) {
    super(`${variable}: ${reason}`);
    this.name = "ConfigError";
  }
}

type Environment = Readonly<Record<string, string | undefined>>;

const SECRET_MIN_BYTES = 32;
const STRICT_BASE64 = /^[A-Za-z0-9+/]+={0,2}$/;
const LOOPBACK = new Set(["localhost", "127.0.0.1", "[::1]"]);

function optional(env: Environment, variable: string): string | null {
  const value = env[variable]?.trim();
  return value === undefined || value === "" ? null : value;
}

function required(env: Environment, variable: string): string {
  const value = optional(env, variable);
  if (value === null) {
    throw new ConfigError(variable, "is not set");
  }
  return value;
}

function url(variable: string, value: string): URL {
  if (!URL.canParse(value)) {
    throw new ConfigError(variable, "is not an absolute URL");
  }
  const parsed = new URL(value);
  if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
    throw new ConfigError(variable, "must be http or https");
  }
  return parsed;
}

function cloudOrigin(env: Environment): string | null {
  const variable = "ORCHESTER_ADMIN_CLOUD_URL";
  const value = optional(env, variable);
  if (value === null) {
    return null;
  }
  const parsed = url(variable, value);
  // The browser is sent there and back; plain http is for a cloud on this computer (tests and development).
  if (parsed.protocol === "http:" && !LOOPBACK.has(parsed.hostname)) {
    throw new ConfigError(variable, "must be https unless it is a loopback address");
  }
  return parsed.origin;
}

function secret(env: Environment): Uint8Array {
  const variable = "ORCHESTER_ADMIN_SECRET";
  const text = required(env, variable);
  // Node's decoder skips characters it does not know; a strict check stops a mistyped secret from becoming a shorter one.
  if (!STRICT_BASE64.test(text) || text.length % 4 !== 0) {
    throw new ConfigError(variable, "must be standard base64 with its padding");
  }
  const bytes = Buffer.from(text, "base64");
  if (bytes.length < SECRET_MIN_BYTES) {
    throw new ConfigError(variable, `must be at least ${SECRET_MIN_BYTES} bytes in base64`);
  }
  return new Uint8Array(bytes);
}

export function readConfig(env: Environment): Config {
  const variable = "ORCHESTER_ADMIN_SERVER_URL";
  const api = url(variable, required(env, variable));
  return {
    apiBase: `${api.origin}${api.pathname.replace(/\/+$/, "")}/api/v1`,
    secret: secret(env),
    cloudOrigin: cloudOrigin(env),
  };
}

let loaded: Config | undefined;

/** The process's configuration, read once; an invalid environment throws a `ConfigError` naming the variable. */
export function config(): Config {
  loaded ??= readConfig(process.env);
  return loaded;
}
