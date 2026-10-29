//! What a git command does beyond the files it names ([`super::RunRoots`]): a push to a remote, a change to a ref
//! other than the run's branch (another branch, a tag, the stash, notes, bisect's refs, what `HEAD` points at), or a
//! write of the repository's shared state in the main checkout's `.git` (its settings, its worktrees, `gc`). The
//! run's branch, its worktree's `HEAD` and index and new objects are the run's own. A form this cannot follow (an
//! alias or unknown subcommand, a setting given with `-c`, `--namespace`, git under a wrapper, `update-ref --stdin`)
//! cannot be told.

use std::path::Path;

use crate::permission::category::own_commit;

/// One effect of a git command beyond its files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum GitEffect {
  Push,
  /// A ref other than the run's branch: a branch by its short name, any other ref by its full name.
  Ref(String),
  /// The repository's shared state, in the main checkout's `.git`.
  Shared,
  /// What it changes cannot be told.
  Unread,
}

/// Subcommands that change no ref but the run's branch and its worktree's `HEAD`, and no shared state.
const OWN: [&str; 59] = [
  "add",
  "am",
  "apply",
  "archive",
  "blame",
  "cat-file",
  "check-attr",
  "check-ignore",
  "check-ref-format",
  "cherry",
  "cherry-pick",
  "clean",
  "clone",
  "commit",
  "commit-tree",
  "count-objects",
  "describe",
  "diff",
  "diff-files",
  "diff-index",
  "diff-tree",
  "difftool",
  "for-each-ref",
  "format-patch",
  "fsck",
  "grep",
  "hash-object",
  "help",
  "init",
  "log",
  "ls-files",
  "ls-remote",
  "ls-tree",
  "merge",
  "merge-base",
  "merge-file",
  "mergetool",
  "mktree",
  "mv",
  "name-rev",
  "range-diff",
  "read-tree",
  "reset",
  "restore",
  "rev-list",
  "rev-parse",
  "revert",
  "rm",
  "shortlog",
  "show",
  "show-ref",
  "sparse-checkout",
  "status",
  "update-index",
  "var",
  "verify-commit",
  "verify-tag",
  "version",
  "write-tree",
];

/// Subcommands that write the repository's shared objects or settings.
const SHARED: [&str; 5] = ["gc", "prune", "repack", "pack-refs", "maintenance"];

/// Options git takes before its subcommand that take the next word as their value.
const GLOBAL_VALUES: [&str; 3] = ["-C", "--git-dir", "--work-tree"];

/// Options before the subcommand that change what git runs or which refs it names.
const GLOBAL_UNREAD: [&str; 4] = ["-c", "--config-env", "--namespace", "--exec-path="];

/// The effect of git run with `args` (the words after `git`), for the run's `branch` in `worktree`; `None` for none.
pub(super) fn effect(args: &[String], branch: &str, worktree: &Path) -> Option<GitEffect> {
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  let mut at = 0;
  while let Some(arg) = args.get(at) {
    match *arg {
      "-c" if args.get(at + 1).is_some_and(|setting| setting.starts_with("user.")) => at += 2,
      arg if GLOBAL_UNREAD.iter().any(|option| arg.starts_with(option)) => return Some(GitEffect::Unread),
      arg if GLOBAL_VALUES.contains(&arg) => at += 2,
      arg if arg.starts_with('-') => at += 1,
      _ => break,
    }
  }
  let sub = *args.get(at)?;
  let refs = Refs { branch, worktree };
  refs.of(sub, &args[at + 1..])
}

/// The run's branch, and the worktree where a `checkout` operand may name a file.
struct Refs<'a> {
  branch: &'a str,
  worktree: &'a Path,
}

impl Refs<'_> {
  fn of(&self, sub: &str, rest: &[&str]) -> Option<GitEffect> {
    match sub {
      "push" => Some(GitEffect::Push),
      "branch" => self.branch(rest),
      "tag" => tag(rest),
      "checkout" => self.checkout(rest),
      "switch" => self.switch(rest),
      "stash" => stash(rest),
      "notes" => notes(rest),
      "update-ref" => self.update_ref(rest),
      "symbolic-ref" => self.symbolic_ref(rest),
      "fetch" | "pull" => self.fetched(rest),
      "rebase" => self.rebase(rest),
      "bisect" => (!matches!(rest.first(), Some(&("log" | "visualize" | "view")))).then(|| refs("refs/bisect")),
      "replace" => operands(rest, &[]).first().map(|_| refs("refs/replace")),
      "config" => config(rest),
      "remote" => remote(rest),
      "reflog" => matches!(rest.first(), Some(&("expire" | "delete"))).then_some(GitEffect::Shared),
      "worktree" => self.worktree(rest),
      "submodule" | "lfs" => {
        (!matches!(rest.first(), Some(&("status" | "summary" | "ls-files")))).then_some(GitEffect::Shared)
      }
      sub if SHARED.contains(&sub) => Some(GitEffect::Shared),
      sub if OWN.contains(&sub) => None,
      _ => Some(GitEffect::Unread),
    }
  }

  /// `name` as a ref this changes: `None` for the run's branch.
  fn other(&self, name: &str) -> Option<GitEffect> {
    let short = name.strip_prefix("refs/heads/").unwrap_or(name);
    (short != self.branch).then(|| GitEffect::Ref(short.to_owned()))
  }

  /// The first of `names` that is not the run's branch.
  fn first_other(&self, names: &[&str]) -> Option<GitEffect> {
    names.iter().find_map(|name| self.other(name))
  }

  /// `git branch`: listing changes nothing; deleting, moving, copying or making a branch changes it; setting its
  /// upstream or description writes the shared settings.
  fn branch(&self, rest: &[&str]) -> Option<GitEffect> {
    const VALUES: [&str; 9] = [
      "-u",
      "--set-upstream-to",
      "--contains",
      "--no-contains",
      "--merged",
      "--no-merged",
      "--points-at",
      "--sort",
      "--format",
    ];
    const LISTS: [&str; 18] = [
      "-l",
      "--list",
      "-a",
      "--all",
      "-r",
      "--remotes",
      "-v",
      "-vv",
      "--verbose",
      "--show-current",
      "--contains",
      "--no-contains",
      "--merged",
      "--no-merged",
      "--points-at",
      "--sort",
      "--format",
      "--column",
    ];
    let names = operands(rest, &VALUES);
    if has(
      rest,
      &["-d", "-D", "--delete", "-m", "-M", "--move", "-c", "-C", "--copy"],
    ) {
      return self.first_other(&names);
    }
    if has(
      rest,
      &["-u", "--set-upstream-to", "--unset-upstream", "--edit-description"],
    ) {
      return Some(GitEffect::Shared);
    }
    if has(rest, &LISTS) {
      return None;
    }
    names.first().and_then(|name| self.other(name))
  }

  /// `git checkout -b <new>` makes a branch; `git checkout <branch>` puts another branch in the worktree, whose next
  /// commit changes it. An operand that names a file of the worktree, or a commit of the run's own, changes no ref.
  fn checkout(&self, rest: &[&str]) -> Option<GitEffect> {
    if let Some(name) = value(rest, &["-b", "-B", "--orphan"]) {
      return self.other(name);
    }
    if rest.contains(&"--") || has(rest, &["--detach"]) {
      return None;
    }
    match operands(rest, &["--conflict"]).as_slice() {
      [name] if !own_commit(name) && !self.worktree.join(name).exists() => self.other(name),
      _ => None,
    }
  }

  fn switch(&self, rest: &[&str]) -> Option<GitEffect> {
    if let Some(name) = value(rest, &["-c", "-C", "--create", "--force-create", "--orphan"]) {
      return self.other(name);
    }
    if has(rest, &["-d", "--detach"]) {
      return None;
    }
    match operands(rest, &[]).first() {
      Some(&"-") => Some(GitEffect::Unread),
      Some(name) => self.other(name),
      None => None,
    }
  }

  fn update_ref(&self, rest: &[&str]) -> Option<GitEffect> {
    if has(rest, &["--stdin"]) {
      return Some(GitEffect::Unread);
    }
    operands(rest, &["-m"]).first().and_then(|name| self.other(name))
  }

  /// `git symbolic-ref <name> <ref>` points `name` at `ref`: `HEAD` at another branch is that branch's.
  fn symbolic_ref(&self, rest: &[&str]) -> Option<GitEffect> {
    let names = operands(rest, &["-m"]);
    match names.as_slice() {
      [name, ..] if has(rest, &["-d", "--delete"]) => self.other(name),
      ["HEAD", target] => self.other(target),
      [name, _] => self.other(name),
      _ => None,
    }
  }

  /// A fetch or pull changes the refs its refspecs name after `:` (`main:main`); the remote-tracking refs a plain one
  /// updates are the remote's copy, as every fetch keeps them.
  fn fetched(&self, rest: &[&str]) -> Option<GitEffect> {
    const VALUES: [&str; 8] = [
      "--depth",
      "--deepen",
      "--shallow-since",
      "--shallow-exclude",
      "-j",
      "--jobs",
      "-o",
      "--server-option",
    ];
    let names = operands(rest, &VALUES);
    names
      .iter()
      .skip(1)
      .filter_map(|refspec| refspec.split_once(':').map(|(_, to)| to))
      .filter(|to| !to.is_empty())
      .find_map(|to| self.other(to))
  }

  /// `git rebase <upstream> <branch>` checks out `branch` and rebases it; `--update-refs` moves the branches between.
  fn rebase(&self, rest: &[&str]) -> Option<GitEffect> {
    if has(rest, &["--update-refs"]) {
      return Some(GitEffect::Unread);
    }
    match operands(
      rest,
      &["--onto", "-s", "--strategy", "-X", "--strategy-option", "-x", "--exec"],
    )
    .as_slice()
    {
      [_, branch, ..] => self.other(branch),
      _ => None,
    }
  }

  /// `git worktree list` reads; every other action changes the repository's worktrees, and `add -b` a branch too.
  fn worktree(&self, rest: &[&str]) -> Option<GitEffect> {
    match rest.first() {
      Some(&"list") => None,
      Some(&"add") => value(rest, &["-b", "-B"])
        .and_then(|name| self.other(name))
        .or(Some(GitEffect::Shared)),
      _ => Some(GitEffect::Shared),
    }
  }
}

fn refs(name: &str) -> GitEffect {
  GitEffect::Ref(name.to_owned())
}

/// `git tag`: listing and verifying change nothing; anything else makes or deletes a tag.
fn tag(rest: &[&str]) -> Option<GitEffect> {
  const VALUES: [&str; 9] = [
    "-m",
    "-F",
    "-u",
    "--cleanup",
    "--sort",
    "--format",
    "--contains",
    "--merged",
    "--points-at",
  ];
  const READS: [&str; 10] = [
    "-l",
    "--list",
    "-v",
    "--verify",
    "--contains",
    "--no-contains",
    "--merged",
    "--no-merged",
    "--points-at",
    "--column",
  ];
  let names = operands(rest, &VALUES);
  if has(rest, &READS) || rest.iter().any(|arg| arg.starts_with("-n")) {
    return None;
  }
  names.first().map(|name| GitEffect::Ref(format!("refs/tags/{name}")))
}

/// `git stash list`, `show`, `apply` and `create` leave `refs/stash`; `branch` makes a branch; the rest change it.
fn stash(rest: &[&str]) -> Option<GitEffect> {
  match operands(rest, &["-m", "--message"]).first() {
    Some(&("list" | "show" | "apply" | "create")) => None,
    Some(&"branch") => operands(rest, &[]).get(1).map(|name| refs(name)),
    _ => Some(refs("refs/stash")),
  }
}

/// `git notes list` and `show` read; the rest change the notes ref (`--ref`, else `refs/notes/commits`).
fn notes(rest: &[&str]) -> Option<GitEffect> {
  let names = operands(rest, &["--ref", "-m", "-F", "-C", "-c"]);
  match names.first() {
    None | Some(&("list" | "show" | "get-ref")) => None,
    Some(_) => Some(refs(value(rest, &["--ref"]).unwrap_or("refs/notes/commits"))),
  }
}

/// `git config` that writes: the worktree's own file is its own; the user's or the system's lie beyond the
/// repository and cannot be told here; any other is the repository's shared settings. Reading changes nothing; an
/// option or subcommand that removes, renames, adds or edits writes, whatever else it names.
fn config(rest: &[&str]) -> Option<GitEffect> {
  const READS: [&str; 7] = [
    "--get",
    "--get-all",
    "--get-regexp",
    "--list",
    "-l",
    "--show-origin",
    "--show-scope",
  ];
  const WRITES: [&str; 8] = [
    "--unset",
    "--unset-all",
    "--remove-section",
    "--rename-section",
    "--add",
    "--replace-all",
    "--edit",
    "-e",
  ];
  let names = operands(rest, &["--file", "-f", "--type", "--default"]);
  let writes = has(rest, &WRITES)
    || matches!(
      names.first(),
      Some(&("set" | "unset" | "rename-section" | "remove-section" | "edit"))
    );
  let reads = has(rest, &READS) || matches!(names.as_slice(), [_] | ["get" | "list", ..]);
  if (reads && !writes) || has(rest, &["--worktree"]) {
    return None;
  }
  if has(rest, &["--global", "--system", "--file", "-f"]) {
    return Some(GitEffect::Unread);
  }
  Some(GitEffect::Shared)
}

/// `git remote` lists or fetches; adding, removing or changing one writes the shared settings.
fn remote(rest: &[&str]) -> Option<GitEffect> {
  match rest.first() {
    None | Some(&("-v" | "--verbose" | "show" | "get-url" | "update")) => None,
    Some(_) => Some(GitEffect::Shared),
  }
}

/// Whether `args` hold one of `options`, alone or as `--option=value`.
fn has(args: &[&str], options: &[&str]) -> bool {
  args.iter().take_while(|arg| **arg != "--").any(|arg| {
    options
      .iter()
      .any(|option| arg == option || arg.strip_prefix(option).is_some_and(|value| value.starts_with('=')))
  })
}

/// The value `args` give the first of `options`: the next word, or after `=`.
fn value<'a>(args: &[&'a str], options: &[&str]) -> Option<&'a str> {
  let mut words = args.iter().take_while(|arg| **arg != "--");
  while let Some(arg) = words.next() {
    for option in options {
      if arg == option {
        return words.next().copied();
      }
      if let Some(value) = arg.strip_prefix(option).and_then(|rest| rest.strip_prefix('=')) {
        return Some(value);
      }
    }
  }
  None
}

/// The operands of `args`: words that are no option and no value of one of `values`, and every word after `--`.
fn operands<'a>(args: &[&'a str], values: &[&str]) -> Vec<&'a str> {
  let mut operands = Vec::new();
  let mut words = args.iter();
  while let Some(arg) = words.next() {
    match *arg {
      "--" => operands.extend(words.by_ref().copied()),
      arg if values.contains(&arg) => {
        words.next();
      }
      arg if arg.starts_with('-') && arg.len() > 1 => {}
      arg => operands.push(arg),
    }
  }
  operands
}
