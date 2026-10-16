/**
 * How the admin writes fingerprints and initials. Raw data, not words: nothing here is translated.
 */

/** A SHA-256 in hex as the browser shows it, in pairs: `4F:7A:92:C1:0D:8E:51:B3 … 6A:2F:C0:91` (`nNRnL`). */
export function shortFingerprint(sha256: string): string {
  const pairs = sha256.toUpperCase().match(/.{2}/g) ?? [];
  return `${pairs.slice(0, 8).join(":")} … ${pairs.slice(-4).join(":")}`;
}

/** Up to two initials of a name for its avatar (`x5rV0`). */
export function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((word) => Array.from(word)[0]?.toUpperCase() ?? "")
    .join("");
}

/** The host and path of an address without its scheme: `orchester.kontur.dev/kontur`. */
export function bareAddress(address: string): string {
  return address.replace(/^https?:\/\//, "").replace(/\/$/, "");
}
