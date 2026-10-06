//! When auto mode's reviewer may judge a `git push`: only a plain push of the run's own branch to
//! `origin`, as `origin` was when the run started, with nothing else in the line that could change where
//! it goes. Everything else, and anything that cannot be told, goes to the person:
//!
//! - any option before `push` (`-c`, `--config-env`, `-C`, `--git-dir`, …), and any option after it but `-u` /
//!   `--set-upstream`;
//! - a remote other than `origin`, or an `origin` whose push URL is no longer the one recorded at the run's start;
//! - a refspec other than the run's branch (`refs/heads/` allowed), `<branch>:<branch>`, or `HEAD` / `@` while the
//!   worktree has the run's branch checked out and nothing else in the line runs git;
//! - another part of the line that changes git's configuration or remotes (`git config`, `git remote`, git with options
//!   before its subcommand), changes folder, or that Orchester could not read.

use crate::permission::category::{ClassifiedCommand, CommandCategory, CommandPart};

/// The only remote a push may go to without the person.
const OWN_REMOTE: &str = "origin";
/// Programs that change the folder the parts after them run in.
const CHANGES_FOLDER: [&str; 4] = ["cd", "pushd", "chdir", "set-location"];
/// git subcommands that change where a push goes.
const CHANGES_REMOTES: [&str; 2] = ["config", "remote"];

/// What Orchester knows about where a push of the run would go.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PushFacts<'a> {
  /// The run's branch.
  pub branch: Option<&'a str>,
  /// The branch checked out in the run's worktree now; `None` when it is detached or cannot be read.
  pub head: Option<&'a str>,
  /// `origin` still pushes where it pushed when the run started.
  pub origin_kept: bool,
}

/// Programs that run another program they are given.
const WRAPPERS: [&str; 11] = [
  "env", "command", "builtin", "exec", "timeout", "nice", "nohup", "xargs", "sudo", "stdbuf", "time",
];

/// Whether `command` may run git in a form the push rules cannot follow: it names a `GIT_*` variable, runs
/// git through a path or under a wrapper, or names `push` in a part that is not a plain `git push`.
/// Checked on the words of the whole line, so a form the classifier did not read still counts.
pub(super) fn disguised_git(command: &ClassifiedCommand) -> bool {
  let tokens: Vec<&str> = command
    .text()
    .split(|c: char| c.is_whitespace() || ";|&()`$<>\"'{}".contains(c))
    .filter(|token| !token.is_empty())
    .collect();
  let git_like = |token: &str| {
    let name = token.rsplit(['/', '\\']).next().unwrap_or(token).to_lowercase();
    name == "git" || name == "git.exe"
  };
  if tokens.iter().any(|token| token.contains("GIT_")) {
    return true;
  }
  if !tokens.iter().any(|token| git_like(token)) {
    return false;
  }
  let plain_push = |part: &CommandPart| {
    let words = part.words();
    words.first().map(String::as_str) == Some("git") && words.get(1).map(String::as_str) == Some("push")
  };
  tokens.iter().any(|token| git_like(token) && *token != "git")
    || tokens.iter().any(|token| WRAPPERS.contains(token))
    || (tokens.contains(&"push")
      && command
        .parts()
        .iter()
        .any(|part| part.words().iter().any(|word| word.contains("push")) && !plain_push(part)))
}

/// Whether `part` runs `git push`, with whatever options come before it.
pub(super) fn is_push(part: &CommandPart) -> bool {
  part.program() == Some("git") && part.words().iter().skip(1).any(|word| word == "push")
}

/// Whether the push `parts[at]` may go to the reviewer under `facts`.
pub(super) fn reviewable_push(parts: &[CommandPart], at: usize, facts: &PushFacts<'_>) -> bool {
  let Some(branch) = facts.branch.filter(|_| facts.origin_kept) else {
    return false;
  };
  let words = parts[at].words();
  if words.get(1).map(String::as_str) != Some("push") {
    return false;
  }
  let operands: Vec<&str> = words[2..]
    .iter()
    .map(String::as_str)
    .filter(|word| !matches!(*word, "-u" | "--set-upstream"))
    .collect();
  let [remote, refspec] = operands.as_slice() else {
    return false;
  };
  let others: Vec<&CommandPart> = parts
    .iter()
    .enumerate()
    .filter(|(index, _)| *index != at)
    .map(|(_, part)| part)
    .collect();
  let only_git = !others.iter().any(|part| part.program() == Some("git"));
  let head = facts.head.filter(|head| *head == branch && only_git);
  *remote == OWN_REMOTE && own_refspec(refspec, branch, head) && others.iter().all(|part| leaves_push_alone(part))
}

/// Whether `refspec` pushes `branch` to `branch`; `HEAD` and `@` stand for `head`, the run's branch when
/// it is checked out and nothing else in the line runs git.
fn own_refspec(refspec: &str, branch: &str, head: Option<&str>) -> bool {
  let (source, destination) = refspec.split_once(':').unwrap_or((refspec, refspec));
  names(source, head) == Some(branch) && names(destination, head) == Some(branch)
}

/// The branch one side of a refspec names: `HEAD` and `@` stand for `head`.
fn names<'a>(side: &'a str, head: Option<&'a str>) -> Option<&'a str> {
  match side.strip_prefix("refs/heads/").unwrap_or(side) {
    "HEAD" | "@" => head,
    other => Some(other),
  }
}

/// Whether `part`, beside a push in the same line, cannot change where the push goes.
fn leaves_push_alone(part: &CommandPart) -> bool {
  if part.category() == CommandCategory::Unknown || part.runs_script() {
    return false;
  }
  match part.program() {
    Some(program) if CHANGES_FOLDER.contains(&program) => false,
    Some("git") => part
      .subcommand()
      .is_some_and(|sub| part.words().get(1).map(String::as_str) == Some(sub) && !CHANGES_REMOTES.contains(&sub)),
    _ => true,
  }
}
