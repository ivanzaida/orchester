import { createServer } from "node:http";
import type { AddressInfo } from "node:net";

import { describe, expect, it } from "vitest";

import { signUp } from "@/lib/server/cloud";

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

describe("api client without a cloud", () => {
  it("answers unreachable when nothing listens", async () => {
    process.env.ORCHESTER_CLOUD_PUBLIC_URL = "http://localhost:4100";
    process.env.ORCHESTER_CLOUD_API_URL = `http://127.0.0.1:${await freePort()}`;
    process.env.ORCHESTER_CLOUD_WEB_SECRET = Buffer.alloc(32, 9).toString("base64");
    const result = await signUp({ email: "a@example.com", password: "a long password", displayName: "A" }, {});
    expect(result).toEqual({ ok: false, error: { kind: "unreachable" } });
  });
});
