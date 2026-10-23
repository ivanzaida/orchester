//! Git by its subcommand and options (board `U97QE`): reading history and state is Read-only, what
//! changes the worktree, its index or its local branches is Worktree, what reaches a remote is
//! Network, and what rewrites or discards history is History / destructive — `git push` too, although
//! it also uses the network. Only what is understood is classified: the reading subcommands with the
//! options [`known_safe`] lists, `git config` only when it reads, `git remote` only when it lists or
//! updates, and no option that starts a program (`--upload-pack`, `-s <strategy>`), names a template or
//! another git folder, or writes outside (`--output`, `--unsafe-paths`). A global option other than
//! `-C <folder inside>` and the pager switches, a transport that runs a command (`ext::`), and any
//! other subcommand are Unknown.

use super::programs::{Judged, Kind, has_option, operands};
use crate::permission::known_safe;

/// Reading subcommands [`known_safe`] does not rule; their options that write a file or start a
/// program are refused in [`judge_read`].
const OTHER_READS: [&str; 8] = [
  "ls-tree",
  "cat-file",
  "describe",
  "shortlog",
  "rev-list",
  "merge-base",
  "show-ref",
  "name-rev",
];

const CHANGES: [&str; 12] = [
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
];

const REWRITES: [&str; 5] = ["push", "rebase", "filter-branch", "filter-repo", "update-ref"];

/// Subcommands a prefix of which allows only commands like them: they cannot write outside the
/// repository, start a program or rewrite history, whatever arguments follow.
const PREFIX_STAYS: [&str; 14] = [
  "status",
  "add",
  "mv",
  "rm",
  "restore",
  "switch",
  "checkout",
  "rev-parse",
  "ls-files",
  "ls-tree",
  "describe",
  "merge-base",
  "show-ref",
  "name-rev",
];

/// Options that start a program, name a template, another git folder or a file outside, in any
/// subcommand that takes them.
const REFUSED: [&str; 13] = [
  "--upload-pack",
  "--receive-pack",
  "--exec",
  "--template",
  "--separate-git-dir",
  "--config",
  "--strategy",
  "--unsafe-paths",
  "--output",
  "--open-files-in-pager",
  "--ext-diff",
  "--textconv",
  "--filters",
];

/// The subcommand of git's `args` (after its global options), where it starts, and the folder of a
/// `-C`; `None` when a global option configures or redirects git (`-c`, `--git-dir`, `--exec-path`, …).
fn subcommand(args: &[String]) -> Option<(usize, &str, Vec<String>)> {
  let mut index = 0;
  let mut folders = Vec::new();
  while let Some(arg) = args.get(index) {
    match arg.as_str() {
      "-C" => {
        folders.push(args.get(index + 1)?.clone());
        index += 2;
      }
      // Who a commit names; every other setting can name a program git starts (`core.hooksPath`,
      // `core.sshCommand`, an alias) or where it reads and writes.
      "-c"
        if args
          .get(index + 1)
          .is_some_and(|setting| setting.starts_with("user.name=") || setting.starts_with("user.email=")) =>
      {
        index += 2;
      }
      "--no-pager" | "-P" | "--no-optional-locks" | "--paginate" | "-p" => index += 1,
      arg if arg.starts_with('-') => return None,
      arg => return Some((index, arg, folders)),
    }
  }
  None
}

/// `git` with `args`.
pub(super) fn judge(args: &[String]) -> Judged {
  let Some((at, sub, folders)) = subcommand(args) else {
    // `git` alone, or `git --version`, prints; any other global option is not read.
    return if args.iter().all(|arg| arg == "--version") {
      Judged::of(Kind::ReadOnly)
    } else {
      Judged::unknown()
    };
  };
  let rest = &args[at + 1..];
  let refused = rest.iter().any(|arg| {
    REFUSED
      .iter()
      .any(|option| arg == option || arg.strip_prefix(option).is_some_and(|value| value.starts_with('=')))
      || arg.starts_with("ext::")
      || arg.starts_with("fd::")
  });
  let mut judged = if refused {
    Judged::unknown()
  } else {
    judge_subcommand(sub, &args[at..])
  };
  if let [repository] = folders.as_slice() {
    judged.repository = Some(repository.clone());
  }
  judged.inside.extend(folders);
  if PREFIX_STAYS.contains(&sub) {
    judged = judged.prefix_stays();
  }
  judged.subcommand(at, sub.to_owned())
}

/// `sub` with its arguments `with_sub` (the subcommand first).
fn judge_subcommand(sub: &str, with_sub: &[String]) -> Judged {
  let rest = &with_sub[1..];
  match sub {
    "status" | "log" | "show" | "diff" | "rev-parse" | "ls-files" | "blame" | "grep" => known_read(with_sub),
    // Signing runs gpg or ssh-keygen with the user's key.
    "commit" | "merge" | "revert" | "cherry-pick" | "am" | "rebase" if has_option(rest, &["-S", "--gpg-sign"]) => {
      Judged::unknown()
    }
    "tag" if has_option(rest, &["-s", "-u", "--sign", "--local-user"]) => Judged::unknown(),
    "branch" | "tag" if has_option(rest, &["-d", "-D", "--delete", "-f", "--force", "-m", "-M", "-c", "-C"]) => {
      Judged::of(Kind::Destructive)
    }
    "branch" if known_safe::readable_arguments(&argv(with_sub)).is_some() => Judged::of(Kind::ReadOnly),
    "tag" if rest.iter().all(|arg| matches!(arg.as_str(), "-l" | "--list")) => Judged::of(Kind::ReadOnly),
    "branch" | "tag" => Judged::of(Kind::Worktree),
    "stash" => stash(rest),
    "reset" if has_option(rest, &["--hard", "--merge", "--keep"]) => Judged::of(Kind::Destructive),
    "reset" => Judged::of(Kind::Worktree),
    "commit" if has_option(rest, &["--amend"]) => Judged::of(Kind::Destructive),
    "clean" if has_option(rest, &["-n", "--dry-run"]) => Judged::of(Kind::ReadOnly),
    "clean" | "reflog" | "gc" | "prune" => Judged::of(Kind::Destructive),
    "merge" | "cherry-pick" | "revert" | "pull" if has_option(rest, &["-s"]) => Judged::unknown(),
    "config" => config(rest),
    "remote" => remote(rest),
    "worktree" => worktree(rest),
    "init" => Judged::writing(Kind::Writes, first_or_here(rest)),
    "clone" if has_option(rest, &["-u", "-c"]) => Judged::unknown(),
    "clone" => Judged::writing(
      Kind::Network,
      vec![operands(rest).get(1).cloned().unwrap_or_else(|| ".".to_owned())],
    ),
    "apply" | "am" if has_option(rest, &["--directory"]) => Judged::unknown(),
    "fetch" | "pull" => Judged::of(Kind::Network),
    "ls-remote" => ls_remote(rest),
    "submodule" => submodule(rest),
    "lfs" => lfs(rest),
    "notes" => Judged::of(Kind::Worktree),
    sub if OTHER_READS.contains(&sub) => Judged::of(Kind::ReadOnly),
    sub if CHANGES.contains(&sub) => Judged::of(Kind::Worktree),
    sub if REWRITES.contains(&sub) => Judged::of(Kind::Destructive),
    _ => Judged::unknown(),
  }
}

fn argv(with_sub: &[String]) -> Vec<String> {
  std::iter::once("git".to_owned())
    .chain(with_sub.iter().cloned())
    .collect()
}

/// A subcommand [`known_safe`] rules: Read-only with the options it lists, else Unknown.
fn known_read(with_sub: &[String]) -> Judged {
  match known_safe::readable_arguments(&argv(with_sub)) {
    Some(words) => Judged::of(Kind::ReadOnly)
      .known_safe()
      .reading(words.into_iter().map(str::to_owned).collect()),
    None => Judged::unknown(),
  }
}

/// The first operand, or the folder the command runs in.
fn first_or_here(rest: &[String]) -> Vec<String> {
  vec![operands(rest).into_iter().next().unwrap_or_else(|| ".".to_owned())]
}

fn stash(rest: &[String]) -> Judged {
  match rest.first().map(String::as_str) {
    Some("drop" | "clear") => Judged::of(Kind::Destructive),
    Some("list" | "show") => Judged::of(Kind::ReadOnly),
    _ => Judged::of(Kind::Worktree),
  }
}

/// `git config` that only reads: `--get` (and its kin) or `--list`, or one name alone, in the
/// repository's own file. The user's (`--global`) and the system's are read as credential locations;
/// any other file, and every write, is Unknown: a setting can name a program git starts
/// (`core.sshCommand`, `core.pager`, `core.fsmonitor`, an alias with `!`).
fn config(rest: &[String]) -> Judged {
  const READING: [&str; 7] = [
    "--get",
    "--get-all",
    "--get-regexp",
    "--list",
    "-l",
    "--show-origin",
    "--show-scope",
  ];
  const SCOPES: [&str; 2] = ["--global", "--system"];
  const PLAIN: [&str; 6] = ["--local", "--worktree", "--null", "-z", "--name-only", "--includes"];
  let (options, names): (Vec<&String>, Vec<&String>) = rest.iter().partition(|arg| arg.starts_with('-'));
  let options: Vec<&str> = options.into_iter().map(String::as_str).collect();
  if options
    .iter()
    .any(|option| !(READING.contains(option) || SCOPES.contains(option) || PLAIN.contains(option)))
  {
    return Judged::unknown();
  }
  let names: Vec<&str> = names.into_iter().map(String::as_str).collect();
  let reads = match names.as_slice() {
    ["list"] | ["get", _] => true,
    [] => options.iter().any(|option| matches!(*option, "--list" | "-l")),
    [_] | [_, _] => options.iter().any(|option| option.starts_with("--get")) || names.len() == 1,
    _ => false,
  };
  match (reads, options.iter().any(|option| SCOPES.contains(option))) {
    (false, _) => Judged::unknown(),
    (true, false) => Judged::of(Kind::ReadOnly),
    (true, true) => Judged::of(Kind::ReadOnly).secret(),
  }
}

/// `git remote` lists (`-v`, `show`, `get-url`) or updates from the remotes; adding, changing or
/// removing one changes where fetches and pushes go, which is Unknown.
fn remote(rest: &[String]) -> Judged {
  match rest.first().map(String::as_str) {
    None | Some("-v" | "--verbose" | "get-url") => Judged::of(Kind::ReadOnly),
    // `show` asks the remote, unless `-n` keeps it to what is cached; it keeps nothing of the answer.
    Some("show") if rest.iter().any(|arg| arg == "-n") => Judged::of(Kind::ReadOnly),
    Some("show") => Judged::of(Kind::Network).looking_up(),
    Some("update" | "prune") => Judged::of(Kind::Network),
    Some(_) => Judged::unknown(),
  }
}

/// `git ls-remote` asks the remote and prints its answer, keeping nothing, when each option is one of
/// these spelled out; any other option is Unknown, since git takes a shortening of an option's name
/// (`--upload=<command>` is `--upload-pack`, which starts that command).
fn ls_remote(rest: &[String]) -> Judged {
  const PLAIN: [&str; 12] = [
    "-h",
    "--heads",
    "-t",
    "--tags",
    "-b",
    "--branches",
    "--refs",
    "-q",
    "--quiet",
    "--exit-code",
    "--get-url",
    "--symref",
  ];
  let plain = rest
    .iter()
    .filter(|arg| arg.starts_with('-'))
    .all(|option| PLAIN.contains(&option.as_str()) || option.starts_with("--sort="));
  if plain {
    Judged::of(Kind::Network).looking_up()
  } else {
    Judged::unknown()
  }
}

/// `git worktree list` reads; `add` writes its folder; anything else is Unknown.
fn worktree(rest: &[String]) -> Judged {
  match rest.split_first() {
    Some((action, _)) if action == "list" => Judged::of(Kind::ReadOnly),
    Some((action, args)) if action == "add" => Judged::writing(Kind::Writes, operands(args)),
    _ => Judged::unknown(),
  }
}

/// `git submodule status` reads; `update`, `init` and `sync` fetch; `foreach` runs a command and is
/// Unknown like every other action.
fn submodule(rest: &[String]) -> Judged {
  match rest.first().map(String::as_str) {
    None | Some("status" | "summary") => Judged::of(Kind::ReadOnly),
    Some("update" | "init" | "sync") => Judged::of(Kind::Network),
    Some(_) => Judged::unknown(),
  }
}

/// `git lfs pull` and `fetch` reach the network, `ls-files` and `status` read; `install` (which writes
/// the user's configuration) and the rest are Unknown.
fn lfs(rest: &[String]) -> Judged {
  match rest.first().map(String::as_str) {
    Some("pull" | "fetch") => Judged::of(Kind::Network),
    Some("ls-files" | "status") => Judged::of(Kind::ReadOnly),
    _ => Judged::unknown(),
  }
}
