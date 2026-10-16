import { describe, expect, it } from "vitest";

import { csrfToken, originMatches, tokenMatches } from "@/lib/server/csrf";
import { needsHttps, originOf, placeOf } from "@/lib/server/place";
import { newPresession, presessionLive } from "@/lib/server/presession";
import { deriveKey } from "@/lib/server/seal";

const key = deriveKey(new Uint8Array(32).fill(1), "csrf");
const other = deriveKey(new Uint8Array(32).fill(2), "csrf");
const NOW = 1_800_000_000_000;

describe("csrf token", () => {
  const abc = newPresession(NOW);
  const abd = newPresession(NOW);

  it("is the same for the same pre-session and refused for another", () => {
    const token = csrfToken(key, { presession: abc }, NOW);
    expect(tokenMatches(key, { presession: abc }, token, NOW)).toBe(true);
    expect(tokenMatches(key, { presession: abd }, token, NOW)).toBe(false);
  });

  it("binds to the session once there is one", () => {
    const before = csrfToken(key, { presession: abc }, NOW);
    const after = csrfToken(key, { presession: abc, session: "sealed" }, NOW);
    expect(after).not.toEqual(before);
    expect(tokenMatches(key, { presession: abc, session: "sealed" }, before, NOW)).toBe(false);
    expect(tokenMatches(key, { presession: abc, session: "sealed" }, after, NOW)).toBe(true);
  });

  it("is refused with another secret, missing, not text, or without either cookie", () => {
    expect(tokenMatches(key, { presession: abc }, csrfToken(other, { presession: abc }, NOW), NOW)).toBe(false);
    expect(tokenMatches(key, { presession: abc }, null, NOW)).toBe(false);
    expect(csrfToken(key, {}, NOW)).toBeNull();
    expect(tokenMatches(key, {}, "anything", NOW)).toBe(false);
  });

  it("stops working with its pre-session's expiry", () => {
    const token = csrfToken(key, { presession: abc }, NOW);
    expect(presessionLive(abc, NOW + 2 * 60 * 60 * 1000)).toBe(false);
    expect(tokenMatches(key, { presession: abc }, token, NOW + 2 * 60 * 60 * 1000)).toBe(false);
  });
});

function headers(values: Record<string, string>) {
  return { get: (name: string) => values[name] ?? null };
}

describe("the admin's own origin", () => {
  it("is what the server's proxy says the browser used", () => {
    const place = placeOf(headers({ host: "127.0.0.1:3100", "x-forwarded-host": "orchester.kontur.dev", "x-forwarded-proto": "https" }));
    expect(originOf(place)).toBe("https://orchester.kontur.dev");
    expect(originMatches("https://orchester.kontur.dev", originOf(place))).toBe(true);
    expect(originMatches("http://orchester.kontur.dev", originOf(place))).toBe(false);
    expect(originMatches("https://evil.example", originOf(place))).toBe(false);
    expect(originMatches(null, originOf(place))).toBe(false);
  });

  it("falls back to the Host and plain http without the proxy", () => {
    expect(originOf(placeOf(headers({ host: "localhost:4200" })))).toBe("http://localhost:4200");
  });

  it("refuses a host the browser could not have sent", () => {
    expect(placeOf(headers({ host: "evil.example/path?x" })).host).toBe("localhost");
  });
});

describe("HTTPS", () => {
  it("is required except on the server itself", () => {
    expect(needsHttps(placeOf(headers({ "x-forwarded-host": "10.0.4.20:8740", "x-forwarded-proto": "http" })))).toBe(true);
    expect(needsHttps(placeOf(headers({ "x-forwarded-host": "orchester.kontur.dev", "x-forwarded-proto": "https" })))).toBe(false);
    for (const host of ["localhost:8740", "127.0.0.1:8740", "[::1]:8740"]) {
      expect(needsHttps(placeOf(headers({ "x-forwarded-host": host, "x-forwarded-proto": "http" }))), host).toBe(false);
    }
  });
});
