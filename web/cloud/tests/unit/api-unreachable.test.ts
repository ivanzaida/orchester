import { describe, expect, it } from "vitest";

import { signUp } from "@/lib/server/cloud";

describe("api client without a cloud", () => {
  it("answers unreachable when nothing listens", async () => {
    process.env.ORCHESTER_CLOUD_PUBLIC_URL = "http://localhost:4100";
    process.env.ORCHESTER_CLOUD_API_URL = "http://127.0.0.1:4196";
    process.env.ORCHESTER_CLOUD_WEB_SECRET = Buffer.alloc(32, 9).toString("base64");
    const result = await signUp({ email: "a@example.com", password: "a long password", displayName: "A" }, {});
    expect(result).toEqual({ ok: false, error: { kind: "unreachable" } });
  });
});
