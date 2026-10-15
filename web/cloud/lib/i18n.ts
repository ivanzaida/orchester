import en from "@/locales/en.json";

/** The shipped locales; a new one is a new `locales/<locale>.json` with exactly `en.json`'s keys and placeholders. */
export const LOCALES = { en } as const;
export type Locale = keyof typeof LOCALES;
export const DEFAULT_LOCALE: Locale = "en";

type Leaves<T, Prefix extends string = ""> = {
  [K in keyof T & string]: T[K] extends string ? `${Prefix}${K}` : Leaves<T[K], `${Prefix}${K}.`>;
}[keyof T & string];

/** Every key of `en.json`, so a missing key fails the type check instead of showing the raw key. */
export type MessageKey = Leaves<typeof en>;

export type MessageParams = Readonly<Record<string, string | number>>;

/**
 * A text to show later, possibly in a client component: its key and its values. `at` (milliseconds since the epoch)
 * fills the `{{time}}` placeholder with a clock time in the reader's own time zone.
 */
export interface Message {
  readonly key: MessageKey;
  readonly params?: MessageParams;
  readonly at?: number;
}

const PLACEHOLDER = /\{\{(\w+)\}\}/g;

function lookup(locale: Locale, key: MessageKey): string {
  let node: unknown = LOCALES[locale];
  for (const part of key.split(".")) {
    node = (node as Record<string, unknown>)[part];
  }
  if (typeof node !== "string") {
    throw new Error(`missing message ${key} in locale ${locale}`);
  }
  return node;
}

/** The text of `key` with its `{{name}}` placeholders filled; a placeholder without a value is an error. */
export function t(key: MessageKey, params: MessageParams = {}, locale: Locale = DEFAULT_LOCALE): string {
  return lookup(locale, key).replace(PLACEHOLDER, (_, name: string) => {
    const value = params[name];
    if (value === undefined) {
      throw new Error(`message ${key} needs a value for ${name}`);
    }
    return String(value);
  });
}

export function message(key: MessageKey, params?: MessageParams, at?: number): Message {
  return { key, ...(params === undefined ? {} : { params }), ...(at === undefined ? {} : { at }) };
}
