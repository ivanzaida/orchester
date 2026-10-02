//! Git by its subcommand and options (board `U97QE`): reading history and state is Read-only, what
//! changes the worktree, its index or its local branches is Worktree, what reaches a remote is
//! Network, and what rewrites or discards history is History / destructive — `git push` too, although
//! it also uses the network. Global options that run a program or point git elsewhere are not read.

use super::programs::{Judged, Kind, has_option};

const READS: [&str; 16] = [
  "status",
  "diff",
  "log",
  "show",
  "blame",
  "rev-parse",
  "ls-files",
  "ls-tree",
  "cat-file",
  "describe",
  "shortlog",
  "grep",
  "rev-list",
  "merge-base",
  "show-ref",
  "name-rev",
];

const CHANGES: [&str; 15] = [
  "add",
  "commit",
  "mv",
  "rm",
  "restore",
  "checkout",
  "switch",
  "merge",
  "cherry-pick",
  "revert",
  "apply",
  "am",
  "init",
  "notes",
  "worktree",
];

const REMOTE: [&str; 5] = ["fetch", "pull", "clone", "ls-remote", "submodule"];

const REWRITES: [&str; 5] = ["push", "rebase", "filter-branch", "filter-repo", "update-ref"];

/// Subcommands that have a History / destructive form, so a prefix of them would also allow it.
pub(super) const WITH_DESTRUCTIVE_FORMS: [&str; 11] = [
  "push",
  "rebase",
  "reset",
  "branch",
  "tag",
  "stash",
  "clean",
  "commit",
  "reflog",
  "gc",
  "update-ref",
];

/// Global options taking a value; `-C` and `--no-pager` change nothing written.
const GLOBAL_VALUED: [&str; 1] = ["-C"];

/// The subcommand of git's `args` (after its global options) and where it starts; `None` when a
/// global option configures or redirects git (`-c`, `--git-dir`, `--exec-path`, …).
pub(super) fn subcommand(args: &[String]) -> Option<(usize, &str)> {
  let mut index = 0;
  while let Some(arg) = args.get(index) {
    if GLOBAL_VALUED.contains(&arg.as_str()) {
      index += 2;
    } else if matches!(
      arg.as_str(),
      "--no-pager" | "-P" | "--no-optional-locks" | "--paginate" | "-p"
    ) {
      index += 1;
    } else if arg.starts_with('-') {
      return None;
    } else {
      return Some((index, arg));
    }
  }
  None
}

/// `git` with `args`.
pub(super) fn judge(args: &[String]) -> Judged {
  let Some((at, sub)) = subcommand(args) else {
    // `git` alone, or `git --version`, prints; any other global option is not read.
    return if args.iter().all(|arg| matches!(arg.as_str(), "--version" | "--help")) {
      Judged::of(Kind::ReadOnly)
    } else {
      Judged::of(Kind::Unknown)
    };
  };
  let rest = &args[at + 1..];
  Judged::of(kind(sub, rest))
}

fn kind(sub: &str, rest: &[String]) -> Kind {
  match sub {
    "reset" if has_option(rest, &["--hard", "--merge", "--keep"]) => Kind::Destructive,
    "reset" => Kind::Worktree,
    "commit" if has_option(rest, &["--amend"]) => Kind::Destructive,
    "clean" if has_option(rest, &["-n", "--dry-run"]) => Kind::ReadOnly,
    "clean" => Kind::Destructive,
    "branch" | "tag" if has_option(rest, &["-d", "-D", "--delete", "-f", "--force", "-m", "-M", "-c", "-C"]) => {
      Kind::Destructive
    }
    "branch" | "tag" | "stash" | "remote" | "config" | "reflog" if lists(rest) => Kind::ReadOnly,
    "stash"
      if rest
        .first()
        .is_some_and(|action| matches!(action.as_str(), "drop" | "clear")) =>
    {
      Kind::Destructive
    }
    "reflog" | "gc" | "prune" => Kind::Destructive,
    "branch" | "tag" | "stash" | "config" => Kind::Worktree,
    "remote"
      if rest
        .first()
        .is_some_and(|action| matches!(action.as_str(), "update" | "prune")) =>
    {
      Kind::Network
    }
    "remote" => Kind::Worktree,
    "lfs" => Kind::Network,
    sub if READS.contains(&sub) => Kind::ReadOnly,
    sub if CHANGES.contains(&sub) => Kind::Worktree,
    sub if REMOTE.contains(&sub) => Kind::Network,
    sub if REWRITES.contains(&sub) => Kind::Destructive,
    _ => Kind::Unknown,
  }
}

/// Whether the subcommand only lists: no arguments, or only listing options (`-v`, `--list`, `-l`,
/// `--get`, `show`).
fn lists(rest: &[String]) -> bool {
  rest.iter().all(|arg| {
    matches!(
      arg.as_str(),
      "-v"
        | "-vv"
        | "--verbose"
        | "-a"
        | "--all"
        | "-r"
        | "--remotes"
        | "-l"
        | "--list"
        | "--get"
        | "--show-current"
        | "list"
        | "show"
    )
  }) || rest.first().is_some_and(|first| first == "--get")
}
