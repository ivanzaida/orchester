import { t } from "@/lib/i18n";
import type { DeviceInfo } from "@/lib/server/cloud";

/**
 * The name a web session gets in the account's device list, from the browser's `User-Agent`: *Edge on Windows*. The
 * cloud stores it as data, like a desktop's computer name, so it is worded here, at sign-in, in the site's locale;
 * the browser and system names are proper names.
 */
const BROWSERS: readonly [RegExp, string][] = [
  [/Edg\//, "Edge"],
  [/OPR\//, "Opera"],
  [/Firefox\//, "Firefox"],
  [/Chrome\//, "Chrome"],
  [/Safari\//, "Safari"],
];

const SYSTEMS: readonly [RegExp, string][] = [
  [/Windows/, "Windows"],
  [/iPhone|iPad/, "iOS"],
  [/Mac OS X|Macintosh/, "macOS"],
  [/Android/, "Android"],
  [/Linux/, "Linux"],
];

function first(table: readonly [RegExp, string][], userAgent: string): string | undefined {
  return table.find(([pattern]) => pattern.test(userAgent))?.[1];
}

export function webDevice(userAgent: string | null): DeviceInfo {
  const agent = userAgent ?? "";
  const browser = first(BROWSERS, agent) ?? t("device.unknownBrowser");
  const system = first(SYSTEMS, agent);
  return { name: system === undefined ? browser : t("device.webName", { browser, system }), platform: "web" };
}
