import { DEFAULT_LOCALE } from "@/lib/i18n";

/**
 * Clock times and dates as the design writes them (*14:32*, *Oct 13*, *Oct 2046*), in a given time zone
 * (`undefined`: the reader's).
 */
type Zone = string | undefined;

function zoned(zone: Zone) {
  return zone === undefined ? {} : { timeZone: zone };
}

export function clockTime(at: number, zone: Zone): string {
  return new Intl.DateTimeFormat(DEFAULT_LOCALE, { hour: "2-digit", minute: "2-digit", hourCycle: "h23", ...zoned(zone) }).format(at);
}

export function shortDate(at: number, zone: Zone): string {
  return new Intl.DateTimeFormat(DEFAULT_LOCALE, { month: "short", day: "numeric", ...zoned(zone) }).format(at);
}

function dayNumber(at: number, zone: Zone): number {
  const parts = new Intl.DateTimeFormat("en-CA", { dateStyle: "short", ...zoned(zone) }).format(at);
  return Math.round(Date.parse(`${parts}T00:00:00Z`) / 86_400_000);
}

/** Calendar days from `now` to `at` in the zone: 0 today, -1 yesterday, 1 tomorrow. */
export function daysFrom(now: number, at: number, zone: Zone): number {
  return dayNumber(at, zone) - dayNumber(now, zone);
}

export const DAY_MS = 86_400_000;
export const HOUR_MS = 3_600_000;
