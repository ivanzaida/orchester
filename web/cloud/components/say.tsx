"use client";

import { useTimeZone } from "@orchester/ui/hooks/browser";

import { clockTime } from "@/lib/dates";
import { t, type Message } from "@/lib/i18n";

/** The words of a `Message`, its `at` shown as a clock time in the reader's time zone. */
export function Say({ message }: { message: Message }) {
  const zone = useTimeZone();
  const params = message.at === undefined ? message.params : { ...message.params, time: clockTime(message.at, zone) };
  return <>{t(message.key, params)}</>;
}
