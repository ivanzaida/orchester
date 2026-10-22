//! Whether a run's pull request would carry commits of the local base branch its remote doesn't have. A run's
//! branch is cut from the local base, so while the local base is ahead of `<remote>/<base>`, GitHub counts those
//! commits as the pull request's too. Before *Create pull request* pushes anything, git fetches just the base from
//! the link's remote into its remote-tracking branch (`FETCH_HEAD` left alone), then counts the commits the branch
//! carries from the commit the run started from and from the local base that `<remote>/<base>` lacks. The start
//! commit counts even when the local base no longer holds it (rebased by a pull, reset to the remote, deleted).
//! Orchester never pushes the base: the person does, or creates it anyway.

use std::path::Path;

use orchester_core::{
  command::{CommandError, CommandOutput, CommandRunner},
  diff::git::local_git_command,
  repository::{RemoteName, redact_userinfo},
  revision::Revision,
  run::BranchName,
};
use thiserror::Error;

/// How the base branch stands against its remote, for one run's branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseOnRemote {
  /// Every commit of the base the branch carries is on the remote.
  InSync,
  /// The branch carries `commits` commits it started from that the remote's base doesn't have.
  Ahead { commits: u64 },
  /// The remote has no such branch and none was fetched before: nothing to compare with, as before this check.
  NoUpstream,
}

/// Why git could not tell.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum UnpushedError {
  #[error("git did not run: {0}")]
  NotRun(#[from] CommandError),
  #[error("git failed: {0}")]
  Failed(String),
  #[error("git answered `{0}` where a count was expected")]
  Unrecognized(String),
}

/// The branches of one check: the run's `head`, started at `start`, and the `base` it goes into, on `remote`.
#[derive(Clone, Copy)]
pub struct BaseCheck<'a> {
  pub remote: &'a RemoteName,
  pub base: &'a BranchName,
  pub head: &'a BranchName,
  pub start: &'a Revision,
}

/// Fetches `check`'s base from its remote, then counts the commits `head` carries from where it started and from the
/// local base that the remote's base doesn't have, with `git` run by `runner` in the repository at `repo_root`. A
/// fetch that fails (offline, no sign-in, no such branch there) leaves the remote-tracking branch as it was, which is
/// then compared with.
pub fn base_on_remote(
  runner: &dyn CommandRunner,
  (git, repo_root): (&Path, &Path),
  check: BaseCheck<'_>,
) -> Result<BaseOnRemote, UnpushedError> {
  let git = Git { runner, git, repo_root };
  let tracking = format!("refs/remotes/{}/{}", check.remote.as_str(), check.base.as_str());
  git.fetch(check, &tracking);
  let upstream = git.run(&[
    "rev-parse",
    "--verify",
    "--quiet",
    "--end-of-options",
    &format!("{tracking}^{{commit}}"),
  ])?;
  if !upstream.success() {
    if upstream.code == Some(1) {
      tracing::info!(%tracking, "no remote-tracking branch of the base: nothing to compare with");
    } else {
      tracing::warn!(%tracking, reason = %failure(&upstream), "cannot read the base's remote-tracking branch");
    }
    return Ok(BaseOnRemote::NoUpstream);
  }
  let head = format!("refs/heads/{}", check.head.as_str());
  let mut carried = git.shared_with_base(check, &head)?;
  if git.started_on_branch(check.start, &head)? {
    carried.push(check.start.as_str().to_owned());
  }
  if carried.is_empty() {
    return Ok(BaseOnRemote::InSync);
  }
  let mut args = vec!["rev-list", "--count"];
  args.extend(carried.iter().map(String::as_str));
  args.extend(["--not", tracking.as_str()]);
  let counted = git.run(&args)?;
  if !counted.success() {
    return Err(UnpushedError::Failed(failure(&counted)));
  }
  let text = counted.stdout.trim();
  match text.parse::<u64>() {
    Ok(0) => Ok(BaseOnRemote::InSync),
    Ok(commits) => Ok(BaseOnRemote::Ahead { commits }),
    Err(_) => Err(UnpushedError::Unrecognized(text.to_owned())),
  }
}

/// Git in the repository the check runs in.
struct Git<'a> {
  runner: &'a dyn CommandRunner,
  git: &'a Path,
  repo_root: &'a Path,
}

impl Git<'_> {
  fn run(&self, args: &[&str]) -> Result<CommandOutput, UnpushedError> {
    Ok(
      self
        .runner
        .run(&local_git_command(self.git, self.repo_root, args.iter().copied()))?,
    )
  }

  /// Fetches the base into `tracking`; a failure only leaves `tracking` as it was, and is logged.
  fn fetch(&self, check: BaseCheck<'_>, tracking: &str) {
    let refspec = format!("+refs/heads/{}:{tracking}", check.base.as_str());
    let args = [
      "fetch",
      "--quiet",
      "--no-tags",
      "--no-write-fetch-head",
      check.remote.as_str(),
      &refspec,
    ];
    let fetch = local_git_command(self.git, self.repo_root, args).with_env("GIT_TERMINAL_PROMPT", "0");
    let reason = match self.runner.run(&fetch) {
      Ok(output) if output.success() => return,
      Ok(output) => failure(&output),
      Err(error) => error.to_string(),
    };
    tracing::warn!(%reason, "cannot fetch the base branch; comparing with its remote-tracking branch as it is");
  }

  /// The commits `head` shares with the local base at their newest: none when the branch shares none, or the local
  /// base is gone.
  fn shared_with_base(&self, check: BaseCheck<'_>, head: &str) -> Result<Vec<String>, UnpushedError> {
    let base = format!("refs/heads/{}", check.base.as_str());
    let shared = self.run(&["merge-base", "--all", "--end-of-options", &base, head])?;
    if !shared.success() {
      tracing::info!(reason = %failure(&shared), "the run's branch shares no commit with the local base");
      return Ok(Vec::new());
    }
    Ok(shared.stdout.split_whitespace().map(str::to_owned).collect())
  }

  /// Whether `head` still holds the commit the run started from: a branch rebased since does not carry it.
  fn started_on_branch(&self, start: &Revision, head: &str) -> Result<bool, UnpushedError> {
    let held = self.run(&["merge-base", "--is-ancestor", start.as_str(), head])?;
    match held.code {
      Some(0) => Ok(true),
      Some(1) => Ok(false),
      _ => Err(UnpushedError::Failed(failure(&held))),
    }
  }
}

/// What git said when it failed, without credentials.
fn failure(output: &CommandOutput) -> String {
  redact_userinfo(output.stderr.trim())
}
