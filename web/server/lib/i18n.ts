import { createTranslator, type MessageKeys, type MessageParams } from "@orchester/ui/i18n";

import en from "@/locales/en.json";

export type { MessageParams } from "@orchester/ui/i18n";

/** The shipped locales; a new one is a new `locales/<locale>.json` with exactly `en.json`'s keys and placeholders. */
export const LOCALES = { en } as const;
export type Locale = keyof typeof LOCALES;
export const DEFAULT_LOCALE: Locale = "en";

/** Every key of `en.json`, so a missing key fails the type check instead of showing the raw key. */
export type MessageKey = MessageKeys<typeof en>;

/**
 * A text to show later, possibly in a client component: its key and its values. `at` (milliseconds since the epoch)
 * fills the `{{time}}` placeholder with a clock time in the reader's own time zone.
 */
export interface Message {
  readonly key: MessageKey;
  readonly params?: MessageParams;
  readonly at?: number;
}

const translator = createTranslator<typeof LOCALES, MessageKey>(LOCALES, DEFAULT_LOCALE);

export function t(key: MessageKey, params?: MessageParams, locale?: Locale): string {
  return translator.t(key, params, locale);
}

export function message(key: MessageKey, params?: MessageParams, at?: number): Message {
  return { key, ...(params === undefined ? {} : { params }), ...(at === undefined ? {} : { at }) };
}
