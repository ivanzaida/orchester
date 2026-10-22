//! Whether a run's pull request would carry commits of the local base branch its remote doesn't have. A run's
//! branch is cut from the local base, so while the local base is ahead of `<remote>/<base>`, GitHub counts those
//! commits as the pull request's too. Before *Create pull request* pushes anything, git fetches just the base from
//! the link's remote into its remote-tracking branch, then counts the commits the branch shares with the local base
//! that `<remote>/<base>` lacks. Orchester never pushes the base: the person does, or creates it anyway.

use std::path::Path;

use orchester_core::{
  command::{CommandError, CommandOutput, CommandRunner},
  diff::git::local_git_command,
  repository::{RemoteName, redact_userinfo},
  run::BranchName,
};
use thiserror::Error;

/// How the base branch stands against its remote, for one run's branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseOnRemote {
  /// Every commit of the base the branch carries is on the remote.
  InSync,
  /// The branch carries `commits` commits of the local base that the remote's base doesn't have.
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

/// The branches of one check: the run's `head`, the `base` it goes into, on `remote`.
#[derive(Clone, Copy)]
pub struct BaseCheck<'a> {
  pub remote: &'a RemoteName,
  pub base: &'a BranchName,
  pub head: &'a BranchName,
}

/// Fetches `check`'s base from its remote, then counts the commits of the local base that `head` carries and the
/// remote's base doesn't, with `git` run by `runner` in the repository at `repo_root`. A fetch that fails (offline,
/// no sign-in, no such branch there) leaves the remote-tracking branch as it was, which is then compared with.
pub fn base_on_remote(
  runner: &dyn CommandRunner,
  (git, repo_root): (&Path, &Path),
  check: BaseCheck<'_>,
) -> Result<BaseOnRemote, UnpushedError> {
  let tracking = format!("refs/remotes/{}/{}", check.remote.as_str(), check.base.as_str());
  let refspec = format!("+refs/heads/{}:{tracking}", check.base.as_str());
  let fetch = local_git_command(
    git,
    repo_root,
    ["fetch", "--quiet", "--no-tags", check.remote.as_str(), &refspec],
  )
  .with_env("GIT_TERMINAL_PROMPT", "0");
  match runner.run(&fetch) {
    Ok(output) if output.success() => {}
    Ok(output) => tracing::warn!(
      reason = %failure(&output),
      "cannot fetch the base branch; comparing with its remote-tracking branch as it is"
    ),
    Err(error) => tracing::warn!(%error, "cannot fetch the base branch; comparing with its remote-tracking branch as it is"),
  }
  let git = |args: &[&str]| -> Result<CommandOutput, UnpushedError> {
    Ok(runner.run(&local_git_command(git, repo_root, args.iter().copied()))?)
  };
  let upstream = format!("{tracking}^{{commit}}");
  if !git(&["rev-parse", "--verify", "--quiet", "--end-of-options", &upstream])?.success() {
    return Ok(BaseOnRemote::NoUpstream);
  }
  let (base, head) = (
    format!("refs/heads/{}", check.base.as_str()),
    format!("refs/heads/{}", check.head.as_str()),
  );
  let shared = git(&["merge-base", "--all", "--end-of-options", &base, &head])?;
  // 1 without output: the branch shares no commit with the local base, so it carries none of its commits.
  if shared.code == Some(1) && shared.stdout.trim().is_empty() {
    return Ok(BaseOnRemote::InSync);
  }
  if !shared.success() {
    return Err(UnpushedError::Failed(failure(&shared)));
  }
  let mut args = vec!["rev-list", "--count"];
  args.extend(shared.stdout.split_whitespace());
  args.extend(["--not", tracking.as_str()]);
  let counted = git(&args)?;
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

/// What git said when it failed, without credentials.
fn failure(output: &CommandOutput) -> String {
  redact_userinfo(output.stderr.trim())
}
