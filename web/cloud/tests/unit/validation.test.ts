import { describe, expect, it } from "vitest";

import {
  displayNameError,
  emailError,
  newPasswordErrors,
  normalizeEmail,
  passwordError,
  signUpErrors,
} from "@/lib/validation";

describe("email", () => {
  it("is kept trimmed and lowercased", () => {
    expect(normalizeEmail("  Mira.Koval@Example.COM ")).toBe("mira.koval@example.com");
  });

  it("needs one @ and a dotted domain, as the cloud does", () => {
    const bad = [
      "mira.koval@example",
      "mira",
      "@example.com",
      "a@b@example.com",
      "a b@example.com",
      "a@.example.com",
      "a@example..com",
      "a@example.com.",
    ];
    for (const value of bad) {
      expect(normalizeEmail(value), value).toBeNull();
    }
    expect(emailError("mira.koval@example")).toEqual({ key: "error.emailInvalid" });
  });

  it("measures length in bytes like the cloud", () => {
    expect(normalizeEmail(`${"a".repeat(64)}@example.com`)).not.toBeNull();
    expect(normalizeEmail(`${"a".repeat(65)}@example.com`)).toBeNull();
    expect(normalizeEmail(`${"é".repeat(33)}@example.com`)).toBeNull();
  });
});

describe("password", () => {
  it("is 12 to 256 characters and says how long it is", () => {
    expect(passwordError("12345678")).toEqual({ key: "error.passwordShort", params: { count: 8 } });
    expect(passwordError("a".repeat(12))).toBeUndefined();
    expect(passwordError("a".repeat(256))).toBeUndefined();
    expect(passwordError("a".repeat(257))).toEqual({ key: "error.passwordLong", params: { count: 257 } });
  });

  it("counts characters, not UTF-16 units", () => {
    expect(passwordError("\u{1F600}".repeat(12))).toBeUndefined();
    expect(passwordError("\u{1F600}".repeat(11))).toEqual({ key: "error.passwordShort", params: { count: 11 } });
  });

  it("must be repeated exactly on reset", () => {
    expect(newPasswordErrors({ password: "123456789", repeat: "123456789012" })).toEqual({
      password: { key: "error.passwordShort", params: { count: 9 } },
      repeat: { key: "error.passwordsDiffer" },
    });
    expect(newPasswordErrors({ password: "a long enough phrase", repeat: "a long enough phrase" })).toEqual({});
  });
});

describe("display name", () => {
  it("is 1 to 100 characters after trimming", () => {
    expect(displayNameError("   ")).toEqual({ key: "error.displayNameEmpty" });
    expect(displayNameError("x".repeat(100))).toBeUndefined();
    expect(displayNameError("x".repeat(101))).toEqual({ key: "error.displayNameLong", params: { count: 101 } });
  });
});

describe("sign-up", () => {
  it("lists only the fields with an error", () => {
    expect(signUpErrors({ email: "mira.koval@example", displayName: "Mira Koval", password: "12345678" })).toEqual({
      email: { key: "error.emailInvalid" },
      password: { key: "error.passwordShort", params: { count: 8 } },
    });
  });
});
