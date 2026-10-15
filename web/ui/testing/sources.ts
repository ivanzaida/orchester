import { readdirSync } from "node:fs";
import { join, relative } from "node:path";

/** The source files of a package that the checks below read: everything under `dirs`, by extension. */
export interface Sources {
  readonly root: string;
  readonly files: (extensions: string[]) => string[];
  readonly relative: (file: string) => string;
}

function walk(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      return entry.name === "node_modules" || entry.name.startsWith(".") ? [] : walk(path);
    }
    return [path];
  });
}

export function sources(root: string, dirs: string[]): Sources {
  return {
    root,
    files: (extensions) =>
      dirs.flatMap((dir) => walk(join(root, dir))).filter((file) => extensions.some((ext) => file.endsWith(ext))),
    relative: (file) => relative(root, file).replaceAll("\\", "/"),
  };
}
