"use client";

import { useTimeZone } from "@orchester/ui/hooks/browser";

import { clockTime, shortDate } from "@/lib/dates";
import { t, type Message } from "@/lib/i18n";

/** The words of a `Message`, its `at` shown as a clock time and its `when` as a date and time, in the reader's zone. */
export function Say({ message }: { message: Message }) {
  const zone = useTimeZone();
  const params = {
    ...message.params,
    ...(message.at === undefined ? {} : { time: clockTime(message.at, zone) }),
    ...(message.when === undefined
      ? {}
      : { when: t("common.dateTime", { date: shortDate(message.when, zone), time: clockTime(message.when, zone) }) }),
  };
  return <>{t(message.key, params)}</>;
}
