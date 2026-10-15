import type { DeviceInfo } from "@/lib/server/cloud";

/**
 * The name a web session gets in the account's device list, from the browser's `User-Agent`: *Edge on Windows*.
 * It is data the cloud stores, like a desktop's computer name, so it is not translated.
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
  const browser = first(BROWSERS, agent) ?? "Web browser";
  const system = first(SYSTEMS, agent);
  return { name: system === undefined ? browser : `${browser} on ${system}`, platform: "web" };
}
