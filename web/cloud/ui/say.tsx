"use client";

import { useSyncExternalStore } from "react";

import { t, type Message } from "@/lib/i18n";

const never = () => () => {};

/** True once the page runs in the browser; `false` while it is rendered on the server and hydrated. */
export function useInBrowser(): boolean {
  return useSyncExternalStore(never, () => true, () => false);
}

/**
 * A clock time like *14:32*. The server does not know the reader's time zone, so it renders UTC and the browser
 * replaces it with its own zone's time right after loading.
 */
export function useClockTime(at: number): string {
  const browser = useInBrowser();
  const format = new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    // The design's 24-hour times (*14:47*), in the reader's own zone.
    hourCycle: "h23",
    ...(browser ? {} : { timeZone: "UTC" }),
  });
  return format.format(at);
}

/** The words of a `Message`, its `at` shown as a clock time in the reader's time zone. */
export function Say({ message }: { message: Message }) {
  const time = useClockTime(message.at ?? 0);
  const params = message.at === undefined ? message.params : { ...message.params, time };
  return <>{t(message.key, params)}</>;
}
