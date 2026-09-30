---
name: codebase-map
description: Use at the start of any task in a repository, before listing or searching files - how to find the right files fast with the codebase index Orchester provides and with rg, instead of dumping folders and whole files.
---

# Orienting in a repository

Every listing or whole-file dump you print stays in your context for the rest of the task. Find the few files
that matter first, then read only the lines you need.

## 1. Use the codebase index

Your prompt names a *codebase index* folder (called `IDX` below; set it first, e.g. `$IDX = '<folder>'` in
PowerShell or `IDX='<folder>'` in bash). It describes this repository at the run's **base revision**:

| File | What it holds | Line format |
|---|---|---|
| `map.md` | packages with their folder, name, description, entry files, test folders, counts and first two levels of modules; folders outside packages | Markdown |
| `tree.txt` | every tracked file with its language and line count | `path<TAB>language<TAB>lines` (`-` for binary files, links, submodules) |
| `symbols.tsv` | one definition per line (types, traits, functions, methods, impls, modules, classes, consts), sorted by name | `name<TAB>path<TAB>line<TAB>kind<TAB>container` (container empty at top level, e.g. `impl Store`, `class Circle`) |
| `outline/<path>.md` | the items of one source file, nested, with line ranges and signatures | `- L<start>-<end> <signature>`, indented under their parent |
| `docs.md` | the headings of the tracked Markdown files | `path:line:## Heading`, as `rg -n` prints matches |
| `manifest.json` | revision, build time, languages and counts | JSON |

Paths are relative to the repository root with `/`, lines start at 1.

Work in this order:

1. Read `map.md` once to see how the repository is split up.
2. Search `symbols.tsv`, `tree.txt` and `docs.md` with `rg`. Do not read them whole.
3. Open `outline/<path>.md` for the few files that matched, to find the item and its line range.
4. Read only that range of the source file.

Check `docs.md` before you look for design notes, architecture docs or conventions.

The index shows the base revision. Anything you or earlier steps of this run changed (`git status --short`,
`git diff --stat`) may have moved, so read those files directly. Files that are not tracked, or are
git-ignored, are not in the index at all.

## 2. Search rules

- Use `rg` (ripgrep) for all searching and listing. It follows `.gitignore` and skips build output.
  Never use `Select-String` over a tree, `Get-ChildItem -Recurse`, `dir /s`, `find .` or `grep -r`.
- Always use `-n` (line numbers) and keep the output small: `-l` for file names only,
  `-m N` for N matches per file, `-M 200` to cut long lines (minified files and fixtures),
  `-g '<glob>'` or `-t <type>` to narrow the files, and a path to narrow the folder.
- Before you read a file, check its length: `tree.txt` has it, or `rg -c '' <file>` prints it.
  Read a whole file only if it is under about 200 lines. Otherwise read ranges of about 50-150 lines around
  what you are looking for.
- Do not print binary files, lock files, generated files, fixtures or logs in full. Search them or read a range.
- Read a file once. Keep what you learned instead of reading it again.

## 3. Examples

PowerShell (Codex on Windows). Put patterns in single quotes so PowerShell leaves `$` and `\` alone; `\t` is
rg's tab:

```powershell
Get-Content "$IDX/map.md"                                    # overview, read once
rg -n -m 20 '^parse_config\t' "$IDX/symbols.tsv"            # where a symbol is defined
rg -n -m 20 '\tmethod\timpl Store$' "$IDX/symbols.tsv"      # methods of one impl
rg -n -m 30 'config' "$IDX/tree.txt"                         # files whose path mentions config
rg -n -i -m 20 'storage' "$IDX/docs.md"                      # documents that cover a topic
Get-Content "$IDX/outline/src/config/load.rs.md"             # items of one file with line ranges
Get-Content src/config/load.rs | Select-Object -Skip 119 -First 60   # lines 120-179
rg -n -m 5 -M 200 'fn parse_config' -t rust                  # search the source itself
rg -l 'ProjectSettings' -g '*.ts' src                        # just the files that mention it
rg --files -g '*.toml'                                       # list files by glob
```

bash (Claude Code, macOS, Linux):

```bash
cat "$IDX/map.md"
rg -n -m 20 '^parse_config\t' "$IDX/symbols.tsv"
rg -n -m 20 '\tmethod\timpl Store$' "$IDX/symbols.tsv"
rg -n -m 30 'config' "$IDX/tree.txt"
rg -n -i -m 20 'storage' "$IDX/docs.md"
cat "$IDX/outline/src/config/load.rs.md"
sed -n '120,179p' src/config/load.rs                         # or your file-read tool with offset/limit
rg -n -m 5 -M 200 'fn parse_config' -t rust
rg -l 'ProjectSettings' -g '*.ts' src
rg --files -g '*.toml'
```

Useful flags: `-w` (whole word), `-F` (literal text, no regex), `-i` (ignore case), `-C 3` (3 lines of
context), `-u` (also search git-ignored files, when you know the file is ignored), `--type-list` (type names).

If `rg` is not found, say so in your output and use `git grep -n '<pattern>' -- <narrowest path>` for the
repository and `git grep --no-index -n '<pattern>' -- "$IDX/symbols.tsv"` for the index. Never recurse over the
whole tree with the shell.

## 4. Before you change code

- Find the definition and its callers: `symbols.tsv` for the definition, then `rg -n -w '<name>'` in the source,
  with `-g` to leave tests out or in.
- Find the tests that cover it: the test folders in `map.md`, then `rg -l '<name>'` over them.
- Read the repository's own agent instructions (`AGENTS.md`, `CLAUDE.md`, contributing docs) if the prompt does
  not already include them. Their headings are in `docs.md`.
