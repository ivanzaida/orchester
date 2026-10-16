import { t } from "@/lib/i18n";

/**
 * The name an admin browser sign-in gets on the server, from the browser's `User-Agent`: *Edge on Windows*. The
 * server stores it as data, like a desktop's computer name, so it is worded here, at sign-in, in the admin's locale;
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

export function browserDevice(userAgent: string | null): string {
  const agent = userAgent ?? "";
  const browser = first(BROWSERS, agent) ?? t("device.unknownBrowser");
  const system = first(SYSTEMS, agent);
  return system === undefined ? browser : t("device.webName", { browser, system });
}
