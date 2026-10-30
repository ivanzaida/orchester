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
/// Programs that set, export or remove the variables the programs after them see.
const SETS_VARIABLES: [&str; 8] = [
  "export", "declare", "typeset", "local", "readonly", "unset", "set", "setx",
];

/// Whether `command` may run git in a form the push rules cannot follow: it sets a `GIT_*` variable, runs git
/// through a path or under a wrapper, or names `push` in a git part that is not a plain `git push`. Only what the
/// line runs is judged, each part's program and a wrapper's words, not the arguments another program reads
/// (`rg -n "GIT"`, `cat docs/git.md`). A line the classifier could not read to the end is judged on all its words
/// ([`names_git`]), so a form it did not read still counts.
pub(super) fn disguised_git(command: &ClassifiedCommand) -> bool {
  let parts = command.parts();
  if parts.is_empty() || parts.iter().any(|part| part.category() == CommandCategory::Unknown) {
    return names_git(command);
  }
  sets_git_variable(command.text()) || parts.iter().any(disguised_part)
}

/// Whether `part`, which the classifier read, runs git in a form the push rules cannot follow: git written with a
/// folder or another name (`git.exe`, `Git`), `push` in a git part that is not a plain `git push`, a wrapper that
/// names git or a `GIT_*` variable, or a program that sets such a variable.
fn disguised_part(part: &CommandPart) -> bool {
  let words = part.words();
  let Some(first) = words.first() else {
    return false;
  };
  if git_like(first) {
    return first != "git"
      || (words.iter().any(|word| word.contains("push")) && !plain_push(part) && !stash_push(part));
  }
  let program = file_name(first);
  let program = program.strip_suffix(".exe").unwrap_or(&program);
  let names_variable = words.iter().any(|word| word.contains("GIT_"));
  if WRAPPERS.contains(&program) {
    return names_variable || words.iter().flat_map(|word| tokens(word)).any(git_like);
  }
  SETS_VARIABLES.contains(&program) && names_variable
}

/// Whether `text` assigns a `GIT_*` variable anywhere (`GIT_DIR=…`, `GIT_DIR+=…`, `$env:GIT_DIR = …`): a lone
/// assignment is no part of the classified line, its value is read into the parts after it.
fn sets_git_variable(text: &str) -> bool {
  text.match_indices("GIT_").any(|(at, prefix)| {
    let rest = text[at + prefix.len()..]
      .trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_')
      .trim_start();
    rest.strip_prefix('+').unwrap_or(rest).starts_with('=')
  })
}

/// Whether the words of the whole line, quoted or not, may run git in a form the push rules cannot follow: for a
/// line the classifier could not read, where any word may be what runs.
fn names_git(command: &ClassifiedCommand) -> bool {
  let tokens: Vec<&str> = tokens(command.text()).collect();
  if tokens.iter().any(|token| token.contains("GIT_")) {
    return true;
  }
  if !tokens.iter().any(|token| git_like(token)) {
    return false;
  }
  tokens.iter().any(|token| git_like(token) && *token != "git")
    || tokens.iter().any(|token| WRAPPERS.contains(token))
    || (tokens.contains(&"push")
      && command
        .parts()
        .iter()
        .any(|part| part.words().iter().any(|word| word.contains("push")) && !plain_push(part) && !stash_push(part)))
}

/// The words of `text` split on whitespace and the shell's operators and quotes.
fn tokens(text: &str) -> impl Iterator<Item = &str> {
  text
    .split(|c: char| c.is_whitespace() || ";|&()`$<>\"'{}".contains(c))
    .filter(|token| !token.is_empty())
}

/// Whether `word` names git, with a folder or in any case: `/usr/bin/git`, `git.exe`, `Git`.
fn git_like(word: &str) -> bool {
  let name = file_name(word);
  name == "git" || name == "git.exe"
}

/// The last segment of the path `word`, in lower case.
fn file_name(word: &str) -> String {
  word.rsplit(['/', '\\']).next().unwrap_or(word).to_lowercase()
}

/// Whether `part` is `git push` written plainly: no option before `push`.
fn plain_push(part: &CommandPart) -> bool {
  let words = part.words();
  words.first().map(String::as_str) == Some("git") && words.get(1).map(String::as_str) == Some("push")
}

/// Whether `part` runs `git push`, with whatever options come before it; `git stash push` keeps its work in the
/// worktree's own stash and pushes nothing.
pub(super) fn is_push(part: &CommandPart) -> bool {
  part.program() == Some("git") && part.words().iter().skip(1).any(|word| word == "push") && !stash_push(part)
}

/// Whether `part` is `git stash push`, written plainly: no option before `stash`.
fn stash_push(part: &CommandPart) -> bool {
  part
    .words()
    .get(..3)
    .is_some_and(|head| head == ["git", "stash", "push"])
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
