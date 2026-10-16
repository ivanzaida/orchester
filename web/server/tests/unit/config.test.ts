import { describe, expect, it } from "vitest";

import { authorizeFields, authorizeStart, backPath, withOutcome } from "@/lib/server/authorize";
import { ConfigError, readConfig } from "@/lib/server/config";
import { browserDevice } from "@/lib/server/device";
import { namedIn } from "@/lib/server/identity";
import { t } from "@/lib/i18n";

const SECRET = Buffer.alloc(32, 9).toString("base64");
const good = {
  ORCHESTER_ADMIN_SERVER_URL: "http://127.0.0.1:8741/",
  ORCHESTER_ADMIN_SECRET: SECRET,
  ORCHESTER_ADMIN_CLOUD_URL: "https://orchester-cloud.lurq.dev/",
};

describe("configuration", () => {
  it("comes from the environment", () => {
    const config = readConfig(good);
    expect(config.apiBase).toBe("http://127.0.0.1:8741/api/v1");
    expect(config.secret).toHaveLength(32);
    expect(config.cloudOrigin).toBe("https://orchester-cloud.lurq.dev");
  });

  it("has no cloud when none is set, and says so instead of failing", () => {
    expect(readConfig({ ...good, ORCHESTER_ADMIN_CLOUD_URL: undefined }).cloudOrigin).toBeNull();
    expect(readConfig({ ...good, ORCHESTER_ADMIN_CLOUD_URL: " " }).cloudOrigin).toBeNull();
  });

  it("names the variable that is wrong", () => {
    expect(() => readConfig({ ...good, ORCHESTER_ADMIN_SERVER_URL: "" })).toThrow(/ORCHESTER_ADMIN_SERVER_URL/);
    expect(() => readConfig({ ...good, ORCHESTER_ADMIN_SECRET: "c2hvcnQ=" })).toThrow(/ORCHESTER_ADMIN_SECRET/);
    expect(() => readConfig({ ...good, ORCHESTER_ADMIN_CLOUD_URL: "http://orchester-cloud.lurq.dev" })).toThrow(ConfigError);
    expect(() => readConfig({ ...good, ORCHESTER_ADMIN_CLOUD_URL: "ftp://x" })).toThrow(/ORCHESTER_ADMIN_CLOUD_URL/);
  });

  it("refuses a secret that is not strict base64, instead of reading what is left of it", () => {
    expect(() => readConfig({ ...good, ORCHESTER_ADMIN_SECRET: `${SECRET.slice(0, 20)}!!${SECRET.slice(20)}` })).toThrow(/base64/);
    expect(() => readConfig({ ...good, ORCHESTER_ADMIN_SECRET: SECRET.replace(/=+$/, "") })).toThrow(/base64/);
  });

  it("allows a plain-http cloud only on this computer", () => {
    expect(readConfig({ ...good, ORCHESTER_ADMIN_CLOUD_URL: "http://127.0.0.1:4220" }).cloudOrigin).toBe("http://127.0.0.1:4220");
  });
});

describe("the trip to the cloud", () => {
  it("posts the purpose, the server or statement, the state and the admin's callback, never in a URL", () => {
    expect(authorizeStart("https://cloud.example")).toBe("https://cloud.example/authorize/start");
    const admin = authorizeFields("https://orchester.kontur.dev", "st", { purpose: "admin", server: "srv" });
    expect(Object.fromEntries(admin)).toEqual({
      purpose: "admin",
      server: "srv",
      state: "st",
      return: "https://orchester.kontur.dev/admin/auth/callback",
    });
    const link = Object.fromEntries(authorizeFields("https://o.example", "st", { purpose: "link", statement: "a.b.c" }));
    expect(link.statement).toBe("a.b.c");
    expect(link.server).toBeUndefined();
  });

  it("comes back to sign-in, the setup or Settings, with a fixed word", () => {
    expect(backPath("admin", "settings")).toBe("/admin/signin");
    expect(backPath("link", "setup")).toBe("/admin/setup/cloud");
    expect(backPath("link", "settings")).toBe("/admin/settings");
    expect(withOutcome("/admin/settings", "link", "done")).toBe("/admin/settings?link=done");
  });
});

describe("identity token for display", () => {
  it("reads the name and email of the payload, and nothing when it cannot", () => {
    const payload = Buffer.from(JSON.stringify({ name: "Dana Ruiz", email: "dana@example.com" })).toString("base64url");
    expect(namedIn(`h.${payload}.s`)).toEqual({ name: "Dana Ruiz", email: "dana@example.com" });
    expect(namedIn("garbage")).toEqual({ name: null, email: null });
    expect(namedIn("h.!!!.s")).toEqual({ name: null, email: null });
  });
});

describe("browser device", () => {
  it("is named after the browser and the system", () => {
    const edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36 Edg/140.0";
    expect(browserDevice(edge)).toBe(t("device.webName", { browser: "Edge", system: "Windows" }));
    expect(browserDevice(null)).toBe(t("device.unknownBrowser"));
  });
});
