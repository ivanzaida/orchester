"use client";

import { useTimeZone } from "@orchester/ui/hooks/browser";

import { clockTime, daysFrom, shortDate } from "@/lib/dates";
import { t, type MessageKey, type MessageParams } from "@/lib/i18n";

/**
 * Dates and times in the reader's own time zone. The server does not know it, so it renders UTC and the browser
 * replaces it right after loading. `now` is the server's time when the page was made, so both agree on *today*; only
 * `near` (*today, 14:02*) needs it.
 */
export type DateFormat = "date" | "time" | "dateTime" | "near";

export interface At {
  readonly at: number;
  readonly format: DateFormat;
}

function format(value: At, now: number | undefined, zone: string | undefined): string {
  const date = shortDate(value.at, zone);
  const time = clockTime(value.at, zone);
  switch (value.format) {
    case "date":
      return date;
    case "time":
      return time;
    case "dateTime":
      return t("common.dateTime", { date, time });
    case "near": {
      const days = now === undefined ? null : daysFrom(now, value.at, zone);
      if (days === 0) {
        return t("common.today", { time });
      }
      return days === -1 ? t("common.yesterday", { time }) : t("common.dateTime", { date, time });
    }
  }
}

/** A sentence whose placeholders in `dates` are moments, written in the reader's zone. */
export function Phrase(props: { k: MessageKey; params?: MessageParams; dates: Readonly<Record<string, At>>; now?: number }) {
  const zone = useTimeZone();
  const filled: Record<string, string | number> = { ...props.params };
  for (const [name, value] of Object.entries(props.dates)) {
    filled[name] = format(value, props.now, zone);
  }
  return <>{t(props.k, filled)}</>;
}
