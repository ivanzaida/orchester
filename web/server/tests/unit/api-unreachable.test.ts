import { describe, expect, it } from "vitest";

import { serverInfo } from "@/lib/server/team/auth";

describe("api client without a server", () => {
  it("answers unreachable when nothing listens", async () => {
    process.env.ORCHESTER_ADMIN_SERVER_URL = "http://127.0.0.1:4296";
    process.env.ORCHESTER_ADMIN_SECRET = Buffer.alloc(32, 9).toString("base64");
    const result = await serverInfo({ place: { proto: "http", host: "localhost" } });
    expect(result).toEqual({ ok: false, error: { kind: "unreachable" } });
  });
});
