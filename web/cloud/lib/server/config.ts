/**
 * The web front's configuration, from the environment only (`docs/architecture/server.md`, "The cloud's web
 * front"): the public base URL, the other origins the site also answers at while it moves, the API's address on the
 * compose network and the secret that signs and seals the cookies.
 */
export interface Config {
  /** The origin people reach the site at, e.g. `https://orchester.cloud`; forms must come from one of `siteOrigins`. */
  readonly publicOrigin: string;
  /**
   * Every origin the site takes forms from: `publicOrigin` first, then `ORCHESTER_CLOUD_EXTRA_ORIGINS`, e.g. the
   * former `https://orchester-cloud.lurq.dev` while the site moves to another domain (`deploy/cloud/README.md`).
   * Each keeps its own host-only cookies.
   */
  readonly siteOrigins: readonly string[];
  /** The API's base, `<ORCHESTER_CLOUD_API_URL>/api/v1`, without a trailing slash. */
  readonly apiBase: string;
  readonly secret: Uint8Array;
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

function required(env: Environment, variable: string): string {
  const value = env[variable]?.trim();
  if (value === undefined || value === "") {
    throw new ConfigError(variable, "is not set");
  }
  return value;
}

function url(env: Environment, variable: string): URL {
  const value = required(env, variable);
  if (!URL.canParse(value)) {
    throw new ConfigError(variable, "is not an absolute URL");
  }
  const parsed = new URL(value);
  if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
    throw new ConfigError(variable, "must be http or https");
  }
  return parsed;
}

/** `__Host-` cookies need a secure context: https, or plain http on this computer for development and tests. */
function secureOrigin(parsed: URL, variable: string): string {
  if (parsed.protocol === "http:" && !LOOPBACK.has(parsed.hostname)) {
    throw new ConfigError(variable, "must be https unless it is a loopback address");
  }
  return parsed.origin;
}

function publicOrigin(env: Environment): string {
  const variable = "ORCHESTER_CLOUD_PUBLIC_URL";
  return secureOrigin(url(env, variable), variable);
}

/**
 * `ORCHESTER_CLOUD_EXTRA_ORIGINS`: comma-separated origins the site also takes forms from, each a scheme, a host and a
 * port alone; unset or empty for none. An origin that is the public one already is listed once.
 */
function extraOrigins(env: Environment, own: string): string[] {
  const variable = "ORCHESTER_CLOUD_EXTRA_ORIGINS";
  const entries = (env[variable] ?? "").split(",").map((entry) => entry.trim()).filter((entry) => entry !== "");
  const origins = entries.map((entry) => {
    if (!URL.canParse(entry)) {
      throw new ConfigError(variable, "lists something that is not an absolute URL");
    }
    const parsed = new URL(entry);
    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
      throw new ConfigError(variable, "must list http or https origins");
    }
    const bare = parsed.pathname === "/" && parsed.search === "" && parsed.hash === "" && parsed.username === "" && parsed.password === "";
    if (!bare) {
      throw new ConfigError(variable, "must list origins alone: a scheme, a host and a port, no path");
    }
    return secureOrigin(parsed, variable);
  });
  return [...new Set(origins)].filter((origin) => origin !== own);
}

function secret(env: Environment): Uint8Array {
  const variable = "ORCHESTER_CLOUD_WEB_SECRET";
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
  const api = url(env, "ORCHESTER_CLOUD_API_URL");
  const own = publicOrigin(env);
  return {
    publicOrigin: own,
    siteOrigins: [own, ...extraOrigins(env, own)],
    apiBase: `${api.origin}${api.pathname.replace(/\/+$/, "")}/api/v1`,
    secret: secret(env),
  };
}

let loaded: Config | undefined;

/** The process's configuration, read once; an invalid environment throws a `ConfigError` naming the variable. */
export function config(): Config {
  loaded ??= readConfig(process.env);
  return loaded;
}
