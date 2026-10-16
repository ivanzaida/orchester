import { fileURLToPath } from "node:url";

import { sources } from "@orchester/ui/testing/sources";

/** The server admin's own source: what the checks read. */
export const ROOT = fileURLToPath(new URL("../..", import.meta.url));
export const src = sources(ROOT, ["app", "components", "lib", "theme"]);
/** The only style sheets that may hold design values; the UI kit's theme holds the rest. */
export const THEME_FILES = ["theme/admin.css"];
