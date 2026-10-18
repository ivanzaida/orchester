//! Pushing a run's branch to the linked repository's remote before a pull request is opened from it
//! (`docs/design/orchester-workspace.md`, "GitHub", rules 4 and 5): `git push <remote>
//! refs/heads/<branch>:refs/heads/<branch>` with git's own credentials. Nothing sets an upstream or
//! changes git config; git asks for nothing in a terminal (there is none), so a missing sign-in is a
//! refusal that names the recovery, *Run `gh auth setup-git` or sign in to git, then retry*.

use std::path::Path;

use crate::{
  command::{Command, CommandError, CommandOutput},
  repository::{RemoteName, redact_userinfo},
  run::BranchName,
};

/// Characters of git's reason kept for the person; the log keeps all of it.
const REASON_CHARS: usize = 200;

/// What the person runs in a terminal when git has no sign-in for the push: `gh` then signs git in with its own
/// account. Orchester shows it to copy and never runs it.
pub const SETUP_GIT: &str = "gh auth setup-git";

/// What git says when the remote did not take the push for want of a sign-in, in lower case: HTTPS without
/// credentials or with wrong ones, an account the repository refuses (403), and SSH without a key it takes.
const AUTHENTICATION: [&str; 9] = [
  "authentication failed",
  "could not read username",
  "could not read password",
  "terminal prompts disabled",
  "invalid username or password",
  "password authentication was removed",
  "permission denied (publickey",
  "the requested url returned error: 401",
  "the requested url returned error: 403",
];

/// What git says when the remote refused the branch itself, in lower case.
const REJECTED: [&str; 2] = ["[rejected]", "[remote rejected]"];

/// Why a push did not happen. Git's words have had URL credentials redacted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushFailure {
  /// The remote wants a sign-in git does not have: the recovery is `gh auth setup-git` or signing in to git.
  Authentication,
  /// The remote refused the branch, e.g. it moved there since (`non-fast-forward`) or a hook declined it.
  Rejected { reason: String },
  /// Git could not push for another reason, as it says it.
  Failed { reason: String },
  /// Git did not run.
  NotRun(CommandError),
}

/// `git push` of `branch` to `remote` from the repository at `repo_root`. Git's messages are read in English, and
/// it never prompts: a sign-in it lacks fails the push instead of waiting for a terminal no one sees.
pub fn git_push_command(git: &Path, repo_root: &Path, remote: &RemoteName, branch: &BranchName) -> Command {
  let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
  Command::new(git, ["push".to_owned(), remote.as_str().to_owned(), refspec])
    .with_current_dir(repo_root)
    .with_env("LC_ALL", "C")
    .with_env("GIT_TERMINAL_PROMPT", "0")
}

/// Reads how `git push` ended.
pub fn parse_git_push(output: &CommandOutput) -> Result<(), PushFailure> {
  if output.success() {
    return Ok(());
  }
  let stderr = redact_userinfo(output.stderr.trim());
  let lower = stderr.to_ascii_lowercase();
  if AUTHENTICATION.iter().any(|pattern| lower.contains(pattern)) {
    return Err(PushFailure::Authentication);
  }
  let rejected = stderr
    .lines()
    .find(|line| REJECTED.iter().any(|pattern| line.contains(pattern)));
  if let Some(line) = rejected {
    // ` ! [rejected]        main -> main (non-fast-forward)`: git's reason is in the parentheses.
    let reason = line
      .rsplit_once('(')
      .and_then(|(_, reason)| reason.strip_suffix(')'))
      .unwrap_or(line.trim());
    return Err(PushFailure::Rejected { reason: short(reason) });
  }
  Err(PushFailure::Failed {
    reason: short(&said(&stderr)),
  })
}

/// Git's first error line without its `fatal:` or `error:`, else its first line.
fn said(stderr: &str) -> String {
  let lines = || stderr.lines().map(str::trim).filter(|line| !line.is_empty());
  let error = lines().find_map(|line| {
    ["fatal: ", "error: "]
      .iter()
      .find_map(|prefix| line.strip_prefix(prefix))
  });
  error.or_else(|| lines().next()).unwrap_or_default().to_owned()
}

fn short(reason: &str) -> String {
  if reason.chars().count() <= REASON_CHARS {
    return reason.to_owned();
  }
  let mut short: String = reason.chars().take(REASON_CHARS).collect();
  short.push('…');
  short
}
