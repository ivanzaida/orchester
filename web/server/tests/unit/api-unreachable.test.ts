import { createServer } from "node:http";
import type { AddressInfo } from "node:net";

import { describe, expect, it } from "vitest";

import { serverInfo } from "@/lib/server/team/auth";

/** A port nothing listens on: one the system just gave out and took back. */
function freePort(): Promise<number> {
  const server = createServer();
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address() as AddressInfo;
      server.close(() => {
        resolve(port);
      });
    });
  });
}

describe("api client without a server", () => {
  it("answers unreachable when nothing listens", async () => {
    process.env.ORCHESTER_ADMIN_SERVER_URL = `http://127.0.0.1:${await freePort()}`;
    process.env.ORCHESTER_ADMIN_SECRET = Buffer.alloc(32, 9).toString("base64");
    const result = await serverInfo({ place: { proto: "http", host: "localhost" } });
    expect(result).toEqual({ ok: false, error: { kind: "unreachable" } });
  });
});
