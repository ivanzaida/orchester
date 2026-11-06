//! The commits a run's work is made of (ORC-19, the review before a queued run merges): those of its branch above where
//! its own work starts, newest first, following only the branch's own line, so a merge of the base it took in is one
//! commit and the base's commits under it are not the run's.

use std::path::Path;

use super::{GitError, git::Git};
use crate::{command::CommandRunner, revision::Revision, time::Timestamp};

/// The most commits [`own_commits`] lists.
pub const MAX_LISTED: usize = 50;

/// A commit of a run's branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoggedCommit {
  pub revision: Revision,
  /// The first line of its message.
  pub subject: String,
  /// When it was committed.
  pub at: Timestamp,
}

/// The commits reachable from `tip` and not from `from` along `tip`'s first parents, in the repository at `repo`,
/// newest first, at most [`MAX_LISTED`].
pub fn own_commits(
  runner: &impl CommandRunner,
  git: &Path,
  (repo, from, tip): (&Path, &Revision, &Revision),
) -> Result<Vec<LoggedCommit>, GitError> {
  let listed = Git::new(runner, git, repo).stdout(&[
    "log",
    "--first-parent",
    "--format=%H%x09%ct%x09%s",
    &format!("--max-count={MAX_LISTED}"),
    &format!("{from}..{tip}"),
  ])?;
  Ok(parse(&listed))
}

/// Whether `ancestor` is in the history of `descendant` in the repository at `repo`, or is it. A commit git does not
/// know, such as one a rebase left behind and git pruned, is in no history.
pub fn is_ancestor(
  runner: &impl CommandRunner,
  git: &Path,
  (repo, ancestor, descendant): (&Path, &Revision, &Revision),
) -> Result<bool, GitError> {
  Git::new(runner, git, repo).is_ancestor(ancestor.as_str(), descendant.as_str())
}

/// The commits `git log --format=%H%x09%ct%x09%s` printed; a line it cannot read is left out.
fn parse(text: &str) -> Vec<LoggedCommit> {
  text
    .lines()
    .filter_map(|line| {
      let mut fields = line.splitn(3, '\t');
      let revision = Revision::new(fields.next()?.trim()).ok()?;
      let seconds: i64 = fields.next()?.trim().parse().ok()?;
      Some(LoggedCommit {
        revision,
        at: Timestamp::from_unix_seconds(seconds),
        subject: fields.next().unwrap_or_default().to_owned(),
      })
    })
    .collect()
}
