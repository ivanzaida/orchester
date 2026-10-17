import { createCipheriv, createDecipheriv, createHmac, hkdfSync, randomBytes, timingSafeEqual } from "node:crypto";

/**
 * Signing and sealing of cookie values with keys derived from `ORCHESTER_CLOUD_WEB_SECRET`, one key per purpose so a
 * value made for one cookie is never accepted as another.
 */
export type KeyPurpose = "csrf" | "session" | "flow" | "authorize" | "challenge";

const IV_BYTES = 12;
const TAG_BYTES = 16;

export function deriveKey(secret: Uint8Array, purpose: KeyPurpose): Buffer {
  return Buffer.from(hkdfSync("sha256", secret, Buffer.alloc(0), `orchester-cloud-web/${purpose}`, 32));
}

export function mac(key: Buffer, value: string): string {
  return createHmac("sha256", key).update(value).digest("base64url");
}

/** Compares two strings in time that does not depend on where they differ. */
export function sameText(a: string, b: string): boolean {
  const left = Buffer.from(a);
  const right = Buffer.from(b);
  return left.length === right.length && timingSafeEqual(left, right);
}

/** `payload.signature`, both base64url: readable by the browser, but not changeable. */
export function sign(key: Buffer, payload: string): string {
  const encoded = Buffer.from(payload).toString("base64url");
  return `${encoded}.${mac(key, encoded)}`;
}

/** The payload of a value `sign` made with this key, or `null` for anything else. */
export function verify(key: Buffer, value: string): string | null {
  const [encoded, signature, extra] = value.split(".");
  if (encoded === undefined || signature === undefined || extra !== undefined) {
    return null;
  }
  return sameText(mac(key, encoded), signature) ? Buffer.from(encoded, "base64url").toString() : null;
}

/** AES-256-GCM: the browser keeps the value but can neither read nor change it. */
export function seal(key: Buffer, plaintext: string): string {
  const iv = randomBytes(IV_BYTES);
  const cipher = createCipheriv("aes-256-gcm", key, iv);
  const body = Buffer.concat([cipher.update(plaintext, "utf8"), cipher.final()]);
  return Buffer.concat([iv, body, cipher.getAuthTag()]).toString("base64url");
}

/** The plaintext `seal` sealed with this key, or `null` when the value was changed or made with another key. */
export function open(key: Buffer, value: string): string | null {
  const bytes = Buffer.from(value, "base64url");
  if (bytes.length <= IV_BYTES + TAG_BYTES) {
    return null;
  }
  const decipher = createDecipheriv("aes-256-gcm", key, bytes.subarray(0, IV_BYTES));
  decipher.setAuthTag(bytes.subarray(bytes.length - TAG_BYTES));
  try {
    return Buffer.concat([decipher.update(bytes.subarray(IV_BYTES, bytes.length - TAG_BYTES)), decipher.final()])
      .toString("utf8");
  } catch {
    // `final` throws only when the tag does not match: a changed or foreign value, which is no session.
    return null;
  }
}
