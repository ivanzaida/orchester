import { readdirSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

/** The web front's own source: what the checks below read. */
export const ROOT = fileURLToPath(new URL("../..", import.meta.url));
const SOURCE_DIRS = ["app", "ui", "lib", "theme"];

function walk(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    return entry.isDirectory() ? walk(path) : [path];
  });
}

export function sourceFiles(extensions: string[]): string[] {
  return SOURCE_DIRS.flatMap((dir) => walk(join(ROOT, dir))).filter((file) =>
    extensions.some((extension) => file.endsWith(extension)),
  );
}

export function relativePath(file: string): string {
  return relative(ROOT, file).replaceAll("\\", "/");
}
