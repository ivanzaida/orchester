import { DEFAULT_LOCALE } from "@/lib/i18n";

/** Clock times and dates as the design writes them (*14:32*, *Oct 13*), in a given time zone (`undefined`: the reader's). */
export function clockTime(at: number, zone: string | undefined): string {
  return new Intl.DateTimeFormat(DEFAULT_LOCALE, {
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    ...(zone === undefined ? {} : { timeZone: zone }),
  }).format(at);
}

export function shortDate(at: number, zone: string | undefined): string {
  return new Intl.DateTimeFormat(DEFAULT_LOCALE, {
    month: "short",
    day: "numeric",
    ...(zone === undefined ? {} : { timeZone: zone }),
  }).format(at);
}

/** Whether two moments fall on the same calendar day in the zone. */
export function sameDay(a: number, b: number, zone: string | undefined): boolean {
  const day = new Intl.DateTimeFormat("en-CA", { dateStyle: "short", ...(zone === undefined ? {} : { timeZone: zone }) });
  return day.format(a) === day.format(b);
}
