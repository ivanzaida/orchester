/**
 * Two-factor authentication as the Account page shows it (`docs/cloud-api.md`, "Two-factor authentication"), as plain
 * data the server loads and the browser renders. Safe in the browser: the secret and the recovery codes appear only in
 * the answers of the forms that make them.
 */

export interface TwoFactorStatus {
  readonly enabled: boolean;
  /** When it was turned on, in milliseconds since the epoch. */
  readonly enabledAt: number | null;
  readonly recoveryCodesLeft: number;
  /** This browser signed in, stepped up or turned it on with a code: it may accept invites. */
  readonly thisSession: boolean;
}

export const TWO_FACTOR_OFF: TwoFactorStatus = { enabled: false, enabledAt: null, recoveryCodesLeft: 0, thisSession: false };

/** A QR code as one SVG path of its dark modules, on a square of `size` modules that includes the quiet zone. */
export interface QrCode {
  readonly size: number;
  readonly path: string;
}

/** Few enough recovery codes left that the page says so. */
export const FEW_RECOVERY_CODES = 3;

/** The recovery codes as a file: one a line, the way the cloud wrote them. */
export function recoveryCodesText(codes: readonly string[]): string {
  return `${codes.join("\n")}\n`;
}
