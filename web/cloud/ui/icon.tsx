import type { JSX } from "react";

import styles from "./icon.module.css";

/**
 * The design's lucide icons (lucide-static 1.57.0, ISC, `lucide-LICENSE.txt`), drawn inline so the page loads
 * nothing else. Each is the icon's shapes in lucide's 24 × 24 box.
 */
const SHAPES = {
  "arrow-left": [<path key="a" d="m12 19-7-7 7-7" />, <path key="b" d="M19 12H5" />],
  "at-sign": [<circle key="a" cx="12" cy="12" r="4" />, <path key="b" d="M16 8v5a3 3 0 0 0 6 0v-1a10 10 0 1 0-4 8" />],
  check: [<path key="a" d="M20 6 9 17l-5-5" />],
  "circle-alert": [
    <circle key="a" cx="12" cy="12" r="10" />,
    <line key="b" x1="12" x2="12" y1="8" y2="12" />,
    <line key="c" x1="12" x2="12.01" y1="16" y2="16" />,
  ],
  "circle-check": [<circle key="a" cx="12" cy="12" r="10" />, <path key="b" d="m16 9-5.5 5.5L8 12" />],
  eye: [
    <path
      key="a"
      d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"
    />,
    <circle key="b" cx="12" cy="12" r="3" />,
  ],
  "eye-off": [
    <path key="a" d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49" />,
    <path key="b" d="M14.084 14.158a3 3 0 0 1-4.242-4.242" />,
    <path key="c" d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143" />,
    <path key="d" d="m2 2 20 20" />,
  ],
  "link-2-off": [
    <path key="a" d="M9 17H7A5 5 0 0 1 7 7" />,
    <path key="b" d="M15 7h2a5 5 0 0 1 4 8" />,
    <line key="c" x1="8" x2="12" y1="12" y2="12" />,
    <line key="d" x1="2" x2="22" y1="2" y2="22" />,
  ],
  "loader-circle": [<path key="a" d="M21 12a9 9 0 1 1-6.219-8.56" />],
  lock: [
    <rect key="a" width="18" height="11" x="3" y="11" rx="2" ry="2" />,
    <path key="b" d="M7 11V7a5 5 0 0 1 10 0v4" />,
  ],
  "log-in": [
    <path key="a" d="m10 17 5-5-5-5" />,
    <path key="b" d="M15 12H3" />,
    <path key="c" d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4" />,
  ],
  "log-out": [
    <path key="a" d="m16 17 5-5-5-5" />,
    <path key="b" d="M21 12H9" />,
    <path key="c" d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />,
  ],
  mail: [<path key="a" d="m22 7-8.991 5.727a2 2 0 0 1-2.009 0L2 7" />, <rect key="b" x="2" y="4" width="20" height="16" rx="2" />],
  "mail-check": [
    <path key="a" d="M22 13V6a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v12c0 1.1.9 2 2 2h8" />,
    <path key="b" d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7" />,
    <path key="c" d="m16 19 2 2 4-4" />,
  ],
  "mail-warning": [
    <path key="a" d="M22 10.5V6a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v12c0 1.1.9 2 2 2h12.5" />,
    <path key="b" d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7" />,
    <path key="c" d="M20 14v4" />,
    <path key="d" d="M20 22v.01" />,
  ],
  "monitor-smartphone": [
    <path key="a" d="M18 8V6a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h8" />,
    <path key="b" d="M10 19v-3.96 3.15" />,
    <path key="c" d="M7 19h5" />,
    <rect key="d" width="6" height="10" x="16" y="12" rx="2" />,
  ],
  "refresh-cw": [
    <path key="a" d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" />,
    <path key="b" d="M21 3v5h-5" />,
    <path key="c" d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" />,
    <path key="d" d="M8 16H3v5" />,
  ],
  send: [
    <path
      key="a"
      d="M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z"
    />,
    <path key="b" d="m21.854 2.147-10.94 10.939" />,
  ],
} satisfies Record<string, JSX.Element[]>;

export type IconName = keyof typeof SHAPES;

/** The design's icon sizes: 12 beside small print, 14 in buttons, 16 in fields and notices, 18 in a status tile. */
export type IconSize = "reason" | "button" | "field" | "tile";

export function Icon({ name, size, spin = false }: { name: IconName; size: IconSize; spin?: boolean }) {
  const className = `${styles.icon} ${styles[size]}${spin ? ` ${styles.spin}` : ""}`;
  return (
    <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      {SHAPES[name]}
    </svg>
  );
}
