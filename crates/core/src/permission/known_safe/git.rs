//! The git subcommands that only read, each with the options it may get. Every name listed is an
//! option of that subcommand, so git never reads it as an abbreviation of another one. Left out are,
//! among others, `--help` (starts a help viewer), `--output` (writes a file), `--ext-diff` and
//! `--textconv` (start a configured program), `--open-files-in-pager` (`-O`), `--show-signature`
//! (starts `gpg`), `--orderfile`, `--contents` and every option that reads a file of paths.

use super::options::Options;

/// The git subcommands that are known-safe.
pub const SUBCOMMANDS: [&str; 9] = [
  "status",
  "log",
  "diff",
  "show",
  "rev-parse",
  "ls-files",
  "branch",
  "blame",
  "grep",
];

/// The start of the `git log` format placeholders that verify a signature (`%G?`, `%GS`, …), which
/// starts `gpg`; refused in every argument.
const SIGNATURE_PLACEHOLDER: &str = "%G";

/// Options of the diff output, shared by `diff`, `log` and `show`.
const DIFF_OUTPUT: &[&str] = &[
  "patch",
  "no-patch",
  "stat",
  "shortstat",
  "numstat",
  "dirstat",
  "summary",
  "compact-summary",
  "name-only",
  "name-status",
  "raw",
  "patch-with-stat",
  "patch-with-raw",
  "unified",
  "function-context",
  "word-diff",
  "word-diff-regex",
  "color-words",
  "color",
  "no-color",
  "color-moved",
  "diff-filter",
  "diff-algorithm",
  "minimal",
  "patience",
  "histogram",
  "anchored",
  "indent-heuristic",
  "no-indent-heuristic",
  "find-renames",
  "find-copies",
  "find-copies-harder",
  "no-renames",
  "break-rewrites",
  "irreversible-delete",
  "ignore-all-space",
  "ignore-space-change",
  "ignore-space-at-eol",
  "ignore-blank-lines",
  "ignore-cr-at-eol",
  "ignore-submodules",
  "inter-hunk-context",
  "text",
  "no-ext-diff",
  "no-textconv",
  "abbrev",
  "full-index",
  "binary",
  "relative",
  "no-relative",
  "src-prefix",
  "dst-prefix",
  "no-prefix",
  "exit-code",
  "quiet",
  "check",
  "pickaxe-all",
  "pickaxe-regex",
  "ws-error-highlight",
  "output-indicator-new",
  "output-indicator-old",
  "output-indicator-context",
];
/// Short options of the diff output: `-M`, `-C` and `-B` as flags only, so a value attached to them
/// is refused.
const DIFF_FLAGS: &str = "pusabwWRzMCBD";
/// `-S` and `-G` (pickaxe) and `-U` (context lines) take a value.
const DIFF_VALUED: &str = "SGU";

/// Options that choose and show commits, shared by `log` and `show`.
const REVISIONS: &[&str] = &[
  "oneline",
  "graph",
  "decorate",
  "no-decorate",
  "abbrev-commit",
  "no-abbrev-commit",
  "format",
  "pretty",
  "date",
  "relative-date",
  "all",
  "branches",
  "tags",
  "remotes",
  "author",
  "committer",
  "grep",
  "all-match",
  "invert-grep",
  "regexp-ignore-case",
  "extended-regexp",
  "fixed-strings",
  "basic-regexp",
  "perl-regexp",
  "since",
  "after",
  "until",
  "before",
  "max-count",
  "skip",
  "reverse",
  "first-parent",
  "merges",
  "no-merges",
  "min-parents",
  "max-parents",
  "follow",
  "topo-order",
  "date-order",
  "author-date-order",
  "left-right",
  "left-only",
  "right-only",
  "cherry-pick",
  "cherry-mark",
  "ancestry-path",
  "simplify-by-decoration",
  "full-history",
  "boundary",
  "parents",
  "children",
  "source",
  "full-diff",
  "notes",
  "no-notes",
  "use-mailmap",
  "no-walk",
  "walk-reflogs",
  "expand-tabs",
  "no-expand-tabs",
];
/// Short options of `log` and `show`, with a count alone (`-5`) and `-n5`.
const REVISION_FLAGS: &str = "0123456789pusabwWRzMCBDiEFPg";
const REVISION_VALUED: &str = "nSGU";

const STATUS: Options = Options {
  long: &[&[
    "short",
    "branch",
    "show-stash",
    "porcelain",
    "long",
    "verbose",
    "untracked-files",
    "ignore-submodules",
    "ignored",
    "column",
    "no-column",
    "ahead-behind",
    "no-ahead-behind",
    "renames",
    "no-renames",
    "find-renames",
    "null",
  ]],
  flags: "sbvz",
  valued: "u",
};

const LOG: Options = Options {
  long: &[REVISIONS, DIFF_OUTPUT],
  flags: REVISION_FLAGS,
  valued: REVISION_VALUED,
};

const DIFF: Options = Options {
  long: &[DIFF_OUTPUT, &["cached", "staged", "merge-base", "no-index"]],
  flags: DIFF_FLAGS,
  valued: DIFF_VALUED,
};

const REV_PARSE: Options = Options {
  long: &[&[
    "abbrev-ref",
    "short",
    "verify",
    "quiet",
    "show-toplevel",
    "show-prefix",
    "show-cdup",
    "git-dir",
    "git-common-dir",
    "absolute-git-dir",
    "is-inside-work-tree",
    "is-inside-git-dir",
    "is-bare-repository",
    "is-shallow-repository",
    "symbolic",
    "symbolic-full-name",
    "all",
    "branches",
    "tags",
    "remotes",
    "show-object-format",
  ]],
  flags: "q",
  valued: "",
};

const LS_FILES: Options = Options {
  long: &[&[
    "cached",
    "deleted",
    "modified",
    "others",
    "ignored",
    "stage",
    "unmerged",
    "killed",
    "directory",
    "no-empty-directory",
    "exclude-standard",
    "exclude",
    "full-name",
    "error-unmatch",
    "abbrev",
    "eol",
    "deduplicate",
    "format",
  ]],
  flags: "cdmoiskutvfz",
  valued: "x",
};

const BLAME: Options = Options {
  long: &[&[
    "porcelain",
    "line-porcelain",
    "incremental",
    "root",
    "show-stats",
    "show-name",
    "show-number",
    "show-email",
    "date",
    "abbrev",
    "since",
    "reverse",
    "first-parent",
    "ignore-rev",
    "minimal",
    "color-lines",
    "color-by-age",
    "no-textconv",
  ]],
  flags: "blstenfpwMC",
  valued: "L",
};

const GREP: Options = Options {
  long: &[&[
    "cached",
    "untracked",
    "exclude-standard",
    "no-exclude-standard",
    "text",
    "no-textconv",
    "ignore-case",
    "max-depth",
    "recursive",
    "no-recursive",
    "word-regexp",
    "invert-match",
    "full-name",
    "extended-regexp",
    "basic-regexp",
    "perl-regexp",
    "fixed-strings",
    "line-number",
    "column",
    "files-with-matches",
    "name-only",
    "files-without-match",
    "null",
    "only-matching",
    "count",
    "color",
    "no-color",
    "break",
    "heading",
    "show-function",
    "context",
    "after-context",
    "before-context",
    "function-context",
    "max-count",
    "and",
    "or",
    "not",
    "all-match",
    "quiet",
  ]],
  flags: "aiIwvhHEGPFnlLzocpWqr",
  valued: "eABCm",
};

/// `git branch` options that only list.
const BRANCH_LIST: [&str; 10] = [
  "--list",
  "-l",
  "--show-current",
  "-a",
  "--all",
  "-r",
  "--remotes",
  "-v",
  "-vv",
  "--verbose",
];

/// The arguments of a git command (subcommand first) that may name a file, or `None` when it is not
/// known-safe.
pub(super) fn readable(args: &[String]) -> Option<Vec<&str>> {
  let (subcommand, rest) = args.split_first()?;
  if rest.iter().any(|arg| arg.contains(SIGNATURE_PLACEHOLDER)) {
    return None;
  }
  let options = match subcommand.as_str() {
    "status" => STATUS,
    "log" | "show" => LOG,
    "diff" => DIFF,
    "rev-parse" => REV_PARSE,
    "ls-files" => LS_FILES,
    "blame" => BLAME,
    "grep" => GREP,
    "branch" => return branch_lists(rest).then(|| args.iter().map(String::as_str).collect()),
    _ => return None,
  };
  let mut candidates = vec![subcommand.as_str()];
  candidates.extend(options.readable(rest)?);
  Some(candidates)
}

/// Whether `git branch` with `args` only lists branches: every option lists, and names are patterns
/// only with `--list`.
fn branch_lists(args: &[String]) -> bool {
  let listing = args.iter().any(|arg| arg == "--list" || arg == "-l");
  args.iter().all(|arg| {
    if arg.starts_with('-') {
      BRANCH_LIST.contains(&arg.as_str())
    } else {
      listing
    }
  })
}
