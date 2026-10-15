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
  "app-window": [
    <rect key="a" x="2" y="4" width="20" height="16" rx="2" />,
    <path key="b" d="M10 4v4" />,
    <path key="c" d="M2 8h20" />,
    <path key="d" d="M6 4v4" />,
  ],
  download: [
    <path key="a" d="M12 15V3" />,
    <path key="b" d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />,
    <path key="c" d="m7 10 5 5 5-5" />,
  ],
  "folder-git-2": [
    <path key="a" d="M18 19a5 5 0 0 1-5-5v8" />,
    <path
      key="b"
      d="M9 20H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H20a2 2 0 0 1 2 2v5"
    />,
    <circle key="c" cx="13" cy="12" r="2" />,
    <circle key="d" cx="20" cy="19" r="2" />,
  ],
  globe: [
    <circle key="a" cx="12" cy="12" r="10" />,
    <path key="b" d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" />,
    <path key="c" d="M2 12h20" />,
  ],
  info: [<circle key="a" cx="12" cy="12" r="10" />, <path key="b" d="M12 16v-4" />, <path key="c" d="M12 8h.01" />],
  "key-round": [
    <path
      key="a"
      d="M2.586 17.414A2 2 0 0 0 2 18.828V21a1 1 0 0 0 1 1h3a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h1a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h.172a2 2 0 0 0 1.414-.586l.814-.814a6.5 6.5 0 1 0-4-4z"
    />,
    <circle key="b" cx="16.5" cy="7.5" r=".5" />,
  ],
  "mail-open": [
    <path key="a" d="M21.2 8.4c.5.38.8.97.8 1.6v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V10a2 2 0 0 1 .8-1.6l8-6a2 2 0 0 1 2.4 0l8 6Z" />,
    <path key="b" d="m22 10-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 10" />,
  ],
  server: [
    <rect key="a" width="20" height="8" x="2" y="2" rx="2" ry="2" />,
    <rect key="b" width="20" height="8" x="2" y="14" rx="2" ry="2" />,
    <line key="c" x1="6" x2="6.01" y1="6" y2="6" />,
    <line key="d" x1="6" x2="6.01" y1="18" y2="18" />,
  ],
  "chevron-down": [<path key="a" d="m6 9 6 6 6-6" />],
  "chevron-right": [<path key="a" d="m9 18 6-6-6-6" />],
  clock: [<circle key="a" cx="12" cy="12" r="10" />, <path key="b" d="M12 6v6l4 2" />],
  cloud: [<path key="a" d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z" />],
  "cloud-off": [
    <path key="a" d="M10.94 5.274A7 7 0 0 1 15.71 10h1.79a4.5 4.5 0 0 1 4.222 6.057" />,
    <path key="b" d="M18.796 18.81A4.5 4.5 0 0 1 17.5 19H9A7 7 0 0 1 5.79 5.78" />,
    <path key="c" d="m2 2 20 20" />,
  ],
  copy: [
    <rect key="a" width="14" height="14" x="8" y="8" rx="2" ry="2" />,
    <path key="b" d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />,
  ],
  "folder-plus": [
    <path key="a" d="M12 10v6" />,
    <path key="b" d="M9 13h6" />,
    <path key="c" d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />,
  ],
  "git-branch": [
    <path key="a" d="M15 6a9 9 0 0 0-9 9V3" />,
    <circle key="b" cx="18" cy="6" r="3" />,
    <circle key="c" cx="6" cy="18" r="3" />,
  ],
  laptop: [
    <path key="a" d="M18 5a2 2 0 0 1 2 2v8.526a2 2 0 0 0 .212.897l1.068 2.127a1 1 0 0 1-.9 1.45H3.62a1 1 0 0 1-.9-1.45l1.068-2.127A2 2 0 0 0 4 15.526V7a2 2 0 0 1 2-2z" />,
    <path key="b" d="M20.054 15.987H3.946" />,
  ],
  link: [
    <path key="a" d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />,
    <path key="b" d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />,
  ],
  menu: [<path key="a" d="M4 5h16" />, <path key="b" d="M4 12h16" />, <path key="c" d="M4 19h16" />],
  plus: [<path key="a" d="M5 12h14" />, <path key="b" d="M12 5v14" />],
  search: [<path key="a" d="m21 21-4.34-4.34" />, <circle key="b" cx="11" cy="11" r="8" />],
  shield: [
    <path key="a" d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" />,
  ],
  "shield-alert": [
    <path key="a" d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" />,
    <path key="b" d="M12 8v4" />,
    <path key="c" d="M12 16h.01" />,
  ],
  "shield-question": [
    <path key="a" d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" />,
    <path key="b" d="M9.1 9a3 3 0 0 1 5.82 1c0 2-3 3-3 3" />,
    <path key="c" d="M12 17h.01" />,
  ],
  "trash-2": [
    <path key="a" d="M10 11v6" />,
    <path key="b" d="M14 11v6" />,
    <path key="c" d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />,
    <path key="d" d="M3 6h18" />,
    <path key="e" d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />,
  ],
  "triangle-alert": [
    <path key="a" d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3" />,
    <path key="b" d="M12 9v4" />,
    <path key="c" d="M12 17h.01" />,
  ],
  unlink: [
    <path key="a" d="m18.84 12.25 1.72-1.71h-.02a5.004 5.004 0 0 0-.12-7.07 5.006 5.006 0 0 0-6.95 0l-1.72 1.71" />,
    <path key="b" d="m5.17 11.75-1.71 1.71a5.004 5.004 0 0 0 .12 7.07 5.006 5.006 0 0 0 6.95 0l1.71-1.71" />,
    <line key="c" x1="8" x2="8" y1="2" y2="5" />,
    <line key="d" x1="2" x2="5" y1="8" y2="8" />,
    <line key="e" x1="16" x2="16" y1="19" y2="22" />,
    <line key="f" x1="19" x2="22" y1="16" y2="16" />,
  ],
  "user-plus": [
    <path key="a" d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />,
    <circle key="b" cx="9" cy="7" r="4" />,
    <line key="c" x1="19" x2="19" y1="8" y2="14" />,
    <line key="d" x1="22" x2="16" y1="11" y2="11" />,
  ],
  "user-x": [
    <path key="a" d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />,
    <circle key="b" cx="9" cy="7" r="4" />,
    <line key="c" x1="17" x2="22" y1="8" y2="13" />,
    <line key="d" x1="22" x2="17" y1="8" y2="13" />,
  ],
  users: [
    <path key="a" d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />,
    <path key="b" d="M16 3.128a4 4 0 0 1 0 7.744" />,
    <path key="c" d="M22 21v-2a4 4 0 0 0-3-3.87" />,
    <circle key="d" cx="9" cy="7" r="4" />,
  ],
  "wifi-off": [
    <path key="a" d="M12 20h.01" />,
    <path key="b" d="M8.5 16.429a5 5 0 0 1 7 0" />,
    <path key="c" d="M5 12.859a10 10 0 0 1 5.17-2.69" />,
    <path key="d" d="M19 12.859a10 10 0 0 0-2.007-1.523" />,
    <path key="e" d="M2 8.82a15 15 0 0 1 4.177-2.643" />,
    <path key="f" d="M22 8.82a15 15 0 0 0-11.288-3.764" />,
    <path key="g" d="m2 2 20 20" />,
  ],
  x: [<path key="a" d="M18 6 6 18" />, <path key="b" d="m6 6 12 12" />],
} satisfies Record<string, JSX.Element[]>;

export type IconName = keyof typeof SHAPES;

/**
 * The design's icon sizes: 11 in a done step, 12 beside small print, 13 beside notes and in selects, 14 in buttons,
 * 15 in a list row's glyph, 16 in fields and notices, 18 in a status tile, 20 in an empty state.
 */
export type IconSize = "step" | "reason" | "note" | "button" | "glyph" | "field" | "tile" | "empty";

export function Icon({ name, size, spin = false }: { name: IconName; size: IconSize; spin?: boolean }) {
  const className = `${styles.icon} ${styles[size]}${spin ? ` ${styles.spin}` : ""}`;
  return (
    <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      {SHAPES[name]}
    </svg>
  );
}
