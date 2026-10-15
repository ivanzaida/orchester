import { describe, expect, it } from "vitest";

import { confirmFailure } from "@/lib/confirm";

const NOW = Date.UTC(2026, 9, 15, 14, 32);

describe("a failed confirmation (decision 65)", () => {
  it("is one answer for the row's code and for the code the merged cloud sends", () => {
    const expected = { failed: true, error: { key: "confirm.failed" } };
    expect(confirmFailure({ kind: "refused", status: 401, code: "confirm_failed" }, NOW)).toEqual(expected);
    expect(confirmFailure({ kind: "refused", status: 422, code: "invalid_link" }, NOW)).toEqual(expected);
  });

  it("says how long a link that saw too many wrong passwords waits, and still offers a new link", () => {
    expect(confirmFailure({ kind: "refused", status: 429, code: "rate_limited", retryAfter: 3600 }, NOW)).toEqual({
      failed: true,
      error: { key: "confirm.tooMany", params: { minutes: 60 }, at: NOW + 3_600_000 },
    });
  });

  it("is the cloud's failure, without the new-link offer, when the cloud does not answer", () => {
    expect(confirmFailure({ kind: "unreachable" }, NOW)).toEqual({ failed: false, error: { key: "error.unavailable" } });
  });
});
