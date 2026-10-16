"use client";

import { useSyncExternalStore } from "react";

const never = () => () => {};

/** True once the page runs in the browser; `false` while it is rendered on the server and hydrated. */
export function useInBrowser(): boolean {
  return useSyncExternalStore(never, () => true, () => false);
}

/**
 * The time zone to show times in. The server does not know the reader's zone, so it renders UTC and the browser
 * replaces it with its own zone right after loading (`undefined` is the browser's zone for `Intl`).
 */
export function useTimeZone(): string | undefined {
  return useInBrowser() ? undefined : "UTC";
}
