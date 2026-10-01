---
name: orchester-codebase-index
description: Use at the start of any task in a repository, before listing or searching files - how to find the right files fast with the codebase index Orchester provides and with rg, instead of dumping folders and whole files.
---

# Orienting in a repository

Every listing or whole-file dump you print stays in your context for the rest of the task. Find the few files
that matter first, then read only the lines you need.

## 1. The codebase index

Your prompt names a *codebase index* folder and shows its package and module map. The index describes this
repository at the run's **base revision**:

| File | What it holds | Line format |
|---|---|---|
| `map.md` | packages with their folder, name, description, entry files, test folders, counts and first two levels of modules; folders outside packages | Markdown |
| `tree.txt` | every tracked file with its language and line count | `path<TAB>language<TAB>lines` (`-` for binary files, links, submodules) |
| `symbols.tsv` | the code's own definitions: types, traits, functions, methods, impls, modules, classes, macros; sorted by name | `name<TAB>path<TAB>line<TAB>kind<TAB>container` |
| `symbols-consts.tsv` | the constants and statics of the code, same format | as `symbols.tsv` |
| `symbols-tests.tsv` | every definition in a test folder (`tests/`, `test/`, `__tests__/`, `spec/`), same format | as `symbols.tsv` |
| `outline/<path>.md` | the items of one source file, nested, with line ranges and signatures | `- L<start>-<end> <signature>`, indented under their parent |
| `docs.md` | the headings of the tracked Markdown files | `path:line:## Heading`, as `rg -n` prints matches |

In the symbols files, column 1 is the name, 2 the path, 3 the line, 4 the kind (`struct`, `enum`, `trait`,
`impl`, `function`, `method`, `module`, `type`, `macro`, `class`, `interface`, `namespace`, `union`, `const`,
`static`) and 5 the container, the enclosing item (`impl Store`, `trait Named`, `class Circle`, `module nested`),
empty at top level. Paths are relative to the repository root with `/`; lines start at 1.

Work in this order:

1. Read the map in your prompt (or `map.md`, if the prompt says it was cut) to see how the repository is split up.
2. Search the symbols files, `tree.txt` and `docs.md` with the commands below. Do not read them whole.
3. Open `outline/<path>.md` for the few files that matched, to find the item and its line range.
4. Read only that range of the source file.

Check `docs.md` before you look for design notes, architecture docs or conventions.

The index shows the base revision. Anything you or earlier steps of this run changed (`git status --short`,
`git diff --stat`) may have moved, so read those files directly. Files that are not tracked, or are
git-ignored, are not in the index at all. The index is shared and read-only: never change its files.

Not every source file has an outline or symbols:

- files over 1 MiB, files with a line over 4 KiB (minified or generated code), and the rare file the parser
  did not finish in time have neither: they are only in `tree.txt`, with their line count;
- files whose path is not a plain file name on every system (`:`, `?`, `\`, a trailing dot, a device name like
  `CON`), and files whose path differs from another only in upper and lower case, keep their symbols but have
  no `outline/` file.

For files without symbols, search the source itself with `rg`; for files without an outline, read the lines
the symbols files point to.

## 2. Commands

Set `IDX` to the index folder from your prompt once. Every command below works as written in both shells after
that; only setting `IDX` and reading a file range differ.

PowerShell (Codex on Windows):

```powershell
$IDX = 'C:\repo\.orchester\index\4d2b8e0'
```

bash (Claude Code, macOS, Linux):

```bash
IDX='/repo/.orchester/index/4d2b8e0'
```

Keep patterns in single quotes: PowerShell and bash then pass `$`, `\`, `^` and `|` to rg unchanged, and rg reads
`\t` as a tab. `[^\t]*` stays inside one column, so a pattern anchored with `^` matches only the column you mean,
never a path. A search for a bare word (`rg 'config' "$IDX/symbols.tsv"`) also matches every path that holds it:
anchor it as below.

Symbols by name (column 1):

```
rg -n '^parse_config\t' "$IDX/symbols.tsv"                   # a definition by its exact name
rg -n -i '^[^\t]*config[^\t]*\t' "$IDX/symbols.tsv"          # names that contain a word, any case
rg -n '^(Store|load_store|StoreError)\t' "$IDX/symbols.tsv"  # several names at once
rg -n '^LIMIT\t' "$IDX/symbols.tsv" "$IDX/symbols-consts.tsv" "$IDX/symbols-tests.tsv"   # consts and tests too
```

Symbols by kind (column 4), by path (column 2) and by container (column 5):

```
rg -n '^[^\t]*\t[^\t]*\t[0-9]+\ttrait\t' "$IDX/symbols.tsv"            # every trait
rg -n '^[^\t]*Error\t[^\t]*\t[0-9]+\tenum\t' "$IDX/symbols.tsv"        # enums whose name ends in Error
rg -n '^[^\t]*\tsrc/config/' "$IDX/symbols.tsv"                        # everything defined under a folder
rg -n '^[^\t]*\tsrc/config/load\.rs\t' "$IDX/symbols.tsv"              # everything defined in one file
rg -n '\tmethod\timpl Store$' "$IDX/symbols.tsv"                       # the methods of one impl
```

Files, documents and outlines:

```
rg -n '^[^\t]*config[^\t]*\t' "$IDX/tree.txt"                # files whose path holds a word, with line counts
rg -n '^src/[^\t]*\trust\t' "$IDX/tree.txt"                  # the Rust files under a folder
rg -n -i 'storage' "$IDX/docs.md"                            # documents that cover a topic
rg '^' "$IDX/outline/src/config/load.rs.md"                  # the items of one file with line ranges
```

An outline path is the source path with `.md` appended, under `outline/`.

The source itself:

```
rg -n -m 5 -M 200 'fn parse_config' -t rust                  # a definition
rg -l -w 'ProjectSettings' -g '*.ts' src                     # the files that mention a name
rg --files -g '*.toml'                                       # files by glob
```

Read lines 120-179 of a file:

```powershell
Get-Content src/config/load.rs | Select-Object -Skip 119 -First 60
```

```bash
sed -n '120,179p' src/config/load.rs                         # or your file-read tool with offset/limit
```

Useful flags: `-w` (whole word), `-F` (literal text, no regex), `-i` (ignore case), `-m N` (at most N matches
per file), `-C 3` (3 lines of context), `-M 200` (cut long lines), `-u` (also search git-ignored files, when you
know the file is ignored), `--type-list` (type names).

If `rg` is not found, say so in your output and use `git grep -n '<pattern>' -- <narrowest path>` for the
repository and `git grep --no-index -n -P '<pattern>' -- "$IDX/symbols.tsv"` for the index (`-P` reads `\t` as a
tab). Never recurse over the whole tree with the shell.

## 3. Search rules

- Use `rg` (ripgrep) for all searching and listing. It follows `.gitignore` and skips build output.
  Never use `Select-String` over a tree, `Get-ChildItem -Recurse`, `dir /s`, `find .` or `grep -r`.
- Always use `-n` (line numbers) and keep the output small: `-l` for file names only, `-m N` for N matches per
  file, `-M 200` to cut long lines (minified files and fixtures), `-g '<glob>'` or `-t <type>` to narrow the
  files, and a path to narrow the folder.
- Before you read a file, check its length: `tree.txt` has it, or `rg -c '^' <file>` prints it.
  Read a whole file only if it is under about 200 lines. Otherwise read ranges of about 50-150 lines around
  what you are looking for.
- Do not print binary files, lock files, generated files, fixtures or logs in full. Search them or read a range.
- Read a file once. Keep what you learned instead of reading it again.

## 4. Before you change code

- Find the definition and its callers: the symbols files for the definition, then `rg -n -w '<name>'` in the
  source, with `-g '!**/tests/**'` to leave tests out.
- Find the tests that cover it: `symbols-tests.tsv` and the test folders in the map, then `rg -l -w '<name>'`
  over them.
