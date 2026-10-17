import { randomBytes } from "node:crypto";
import { mkdirSync, readFileSync } from "node:fs";
import type { Server } from "node:http";
import { join, resolve } from "node:path";

import { run, start, stop, waitFor, waitForUrl, type Started } from "./processes";
import { startFront } from "./front";
import { SmtpSink } from "./smtp";

/**
 * Everything the end-to-end run needs, on loopback: PostgreSQL (a throwaway Docker container), Orchester Cloud with a
 * published key set and its web front, the team server with its own certificate, and the server admin. The binaries
 * and both Next.js builds are made beforehand (`npm run test:e2e` says how).
 */
/** `cloudWeb` is the cloud's public front door (`front.ts`); the web front itself listens on `cloudWebApp`. */
export const PORTS = { cloud: 4490, cloudWeb: 4400, cloudWebApp: 4401, server: 4440, serverApi: 4441, admin: 4420, smtp: 4425 } as const;

const ROOT = resolve(import.meta.dirname, "../../../..");
const WEB = join(ROOT, "web");
const EXE = process.platform === "win32" ? ".exe" : "";

export const CLOUD_WEB = `http://localhost:${PORTS.cloudWeb}`;
export const SERVER = `https://127.0.0.1:${PORTS.server}`;
export const SERVER_API = `http://127.0.0.1:${PORTS.serverApi}/api/v1`;
export const CLOUD_API = `http://127.0.0.1:${PORTS.cloud}/api/v1`;

export interface Stack {
  readonly setupCode: string;
  readonly smtp: SmtpSink;
  readonly directory: string;
  readonly processes: readonly Started[];
  readonly serverConfig: Record<string, string>;
  stop(): Promise<void>;
}

function binary(name: string): string {
  return join(ROOT, "target", "debug", `${name}${EXE}`);
}

function next(app: string, port: number, host: string, env: Record<string, string>): Started {
  const cli = join(WEB, "node_modules", "next", "dist", "bin", "next");
  const base = { NODE_ENV: "production", NEXT_TELEMETRY_DISABLED: "1" };
  return start(app, process.execPath, [cli, "start", "-H", host, "-p", String(port)], { ...base, ...env }, join(WEB, app));
}

async function postgres(directory: string): Promise<{ url: string; container: string }> {
  const name = `orchester-sa-e2e-${randomBytes(6).toString("hex")}`;
  const password = randomBytes(16).toString("hex");
  await run("docker", ["run", "-d", "--rm", "--name", name, "-p", "127.0.0.1::5432", "-e", `POSTGRES_PASSWORD=${password}`,
    "postgres:18-alpine"]);
  const port = (await run("docker", ["port", name, "5432/tcp"])).split(/\r?\n/).find((line) => line.startsWith("127.0.0.1:"));
  if (port === undefined) {
    throw new Error("docker did not publish PostgreSQL on 127.0.0.1");
  }
  await waitFor("PostgreSQL", async () => {
    try {
      await run("docker", ["exec", name, "pg_isready", "-h", "127.0.0.1", "-U", "postgres"]);
      return true;
    } catch {
      return null;
    }
  });
  mkdirSync(directory, { recursive: true });
  return { url: `postgres://postgres:${password}@${port}/postgres?sslmode=disable`, container: name };
}

/** The cloud makes its root key and publishes its first key set before it listens. */
async function cloud(database: string, smtp: number): Promise<Started> {
  const env = {
    ORCHESTER_CLOUD_LISTEN: `127.0.0.1:${PORTS.cloud}`,
    ORCHESTER_CLOUD_PUBLIC_URL: CLOUD_WEB,
    ORCHESTER_CLOUD_DATABASE_URL: database,
    ORCHESTER_CLOUD_SMTP_URL: `smtp://127.0.0.1:${smtp}`,
    ORCHESTER_CLOUD_SMTP_FROM: "Orchester <no-reply@cloud.test>",
    ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY: randomBytes(32).toString("hex"),
    // The web front forwards the browser's address.
    ORCHESTER_CLOUD_TRUSTED_PROXIES: "127.0.0.1",
  };
  const started = start("orchester-cloud", binary("orchester-cloud"), ["serve"], env);
  await waitForUrl(`${CLOUD_API}/keys`);
  return started;
}

function serverConfig(directory: string): Record<string, string> {
  return {
    ORCHESTER_SERVER_LISTEN: `127.0.0.1:${PORTS.server}`,
    ORCHESTER_SERVER_DATA_DIR: join(directory, "server-data"),
    ORCHESTER_SERVER_NAME: "Kontur team",
    ORCHESTER_SERVER_TLS: "self_signed",
    ORCHESTER_SERVER_ADMIN_URL: `http://127.0.0.1:${PORTS.admin}`,
    ORCHESTER_SERVER_ADMIN_API_LISTEN: `127.0.0.1:${PORTS.serverApi}`,
  };
}

async function teamServer(config: Record<string, string>): Promise<{ started: Started; setupCode: string }> {
  const started = start("orchester-server", binary("orchester-server"), ["serve"], config);
  const setupCode = await waitFor("the setup code", () => /Setup code: ([0-9A-Z]{4}-[0-9A-Z]{4})/.exec(started.stdout())?.[1] ?? null);
  await waitForUrl(`${SERVER_API}/server`);
  return { started, setupCode };
}

/** Starts the whole run; `cloud: false` is the air-gapped run, with no cloud at all and none named to the admin. */
export async function startStack(options: { cloud: boolean }): Promise<Stack> {
  const directory = join(ROOT, ".tmp", "sa-e2e", `${options.cloud ? "online" : "airgapped"}-${String(Date.now())}`);
  mkdirSync(directory, { recursive: true });
  const smtp = new SmtpSink();
  const processes: Started[] = [];
  const fronts: Server[] = [];
  let container: string | undefined;
  if (options.cloud) {
    await smtp.listen(PORTS.smtp);
    const database = await postgres(directory);
    container = database.container;
    processes.push(await cloud(database.url, PORTS.smtp));
    fronts.push(await startFront(PORTS.cloudWeb, PORTS.cloud, PORTS.cloudWebApp));
    processes.push(next("cloud", PORTS.cloudWebApp, "127.0.0.1", {
      ORCHESTER_CLOUD_PUBLIC_URL: CLOUD_WEB,
      ORCHESTER_CLOUD_API_URL: `http://127.0.0.1:${PORTS.cloud}`,
      ORCHESTER_CLOUD_WEB_SECRET: randomBytes(32).toString("base64"),
    }));
    await waitForUrl(`${CLOUD_WEB}/signin`);
  }
  const config = serverConfig(directory);
  const server = await teamServer(config);
  processes.push(server.started);
  const cloudUrl: Record<string, string> = options.cloud ? { ORCHESTER_ADMIN_CLOUD_URL: CLOUD_WEB } : {};
  processes.push(next("server", PORTS.admin, "127.0.0.1", {
    ORCHESTER_ADMIN_SERVER_URL: `http://127.0.0.1:${PORTS.serverApi}`,
    ORCHESTER_ADMIN_SECRET: randomBytes(32).toString("base64"),
    // Records every outside connection the admin's server side opens (the air-gapped check).
    NODE_OPTIONS: `--import=${pathToUrl(join(import.meta.dirname, "connections.mjs"))}`,
    SA_E2E_CONNECTIONS: join(directory, "admin-connections.log"),
    ...cloudUrl,
  }));
  await waitForUrl(`http://127.0.0.1:${PORTS.admin}/admin`);
  return {
    setupCode: server.setupCode,
    smtp,
    directory,
    processes,
    serverConfig: config,
    async stop() {
      for (const started of [...processes].reverse()) {
        await stop(started);
      }
      smtp.close();
      fronts.forEach((front) => { front.close(); });
      if (container !== undefined) {
        await run("docker", ["rm", "-f", container]);
      }
    },
  };
}

function pathToUrl(path: string): string {
  return `file:///${path.replace(/\\/g, "/").replace(/^\//, "")}`;
}

/** The addresses the admin's server side connected to, as `connections.ts` recorded them. */
export function adminConnections(stack: Stack): string[] {
  try {
    return readFileSync(join(stack.directory, "admin-connections.log"), "utf8").split("\n").filter((line) => line !== "");
  } catch {
    return [];
  }
}

export const BINARY = { server: binary("orchester-server"), cloud: binary("orchester-cloud") };
