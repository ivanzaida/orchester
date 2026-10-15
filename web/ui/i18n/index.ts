/**
 * The i18n helpers both web apps use (server.md, "The web workspace"): a typed `t(key, params)` over the app's own
 * messages. The UI kit has no words; each app passes its locale files here.
 */

/** Every dotted key of a nested messages object whose leaves are strings. */
export type MessageKeys<T, Prefix extends string = ""> = {
  [K in keyof T & string]: T[K] extends string ? `${Prefix}${K}` : MessageKeys<T[K], `${Prefix}${K}.`>;
}[keyof T & string];

export type MessageParams = Readonly<Record<string, string | number>>;

const PLACEHOLDER = /\{\{(\w+)\}\}/g;

export interface Translator<Key extends string, Locale extends string> {
  /** The text of `key` with its `{{name}}` placeholders filled; a key or a value that is missing is an error. */
  readonly t: (key: Key, params?: MessageParams, locale?: Locale) => string;
}

export function createTranslator<Locales extends Record<string, object>, Key extends string>(
  locales: Locales,
  defaultLocale: keyof Locales & string,
): Translator<Key, keyof Locales & string> {
  const lookup = (locale: keyof Locales & string, key: Key): string => {
    let node: unknown = locales[locale];
    for (const part of key.split(".")) {
      node = (node as Record<string, unknown>)[part];
    }
    if (typeof node !== "string") {
      throw new Error(`missing message ${key} in locale ${locale}`);
    }
    return node;
  };
  return {
    t: (key, params = {}, locale = defaultLocale) =>
      lookup(locale, key).replace(PLACEHOLDER, (_, name: string) => {
        const value = params[name];
        if (value === undefined) {
          throw new Error(`message ${key} needs a value for ${name}`);
        }
        return String(value);
      }),
  };
}

/** The `{{name}}` placeholders of a text, sorted: two locales' texts for one key must have the same. */
export function placeholders(text: string): string[] {
  return [...text.matchAll(PLACEHOLDER)].map((match) => match[1] ?? "").sort();
}

type Tree = { readonly [key: string]: string | Tree };

/** A nested messages object as `key → text`. */
export function flatten(tree: object, prefix = ""): Map<string, string> {
  const result = new Map<string, string>();
  for (const [key, value] of Object.entries(tree as Tree)) {
    const path = `${prefix}${key}`;
    if (typeof value === "string") {
      result.set(path, value);
    } else {
      for (const [inner, text] of flatten(value, `${path}.`)) {
        result.set(inner, text);
      }
    }
  }
  return result;
}

/** What differs between a locale and the reference: missing or extra keys, and keys whose placeholders differ. */
export function parityProblems(reference: object, locale: object): string[] {
  const expected = flatten(reference);
  const actual = flatten(locale);
  const problems: string[] = [];
  for (const [key, text] of expected) {
    const other = actual.get(key);
    if (other === undefined) {
      problems.push(`missing ${key}`);
    } else if (placeholders(other).join() !== placeholders(text).join()) {
      problems.push(`placeholders differ in ${key}`);
    }
  }
  for (const key of actual.keys()) {
    if (!expected.has(key)) {
      problems.push(`extra ${key}`);
    }
  }
  return problems;
}
