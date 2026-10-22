//! One pull request as the new app's page shows it (`Pull request · Files changed` `mVTVX`, its states in
//! `c07NAC`): the pull request and its task, the diff of its head and what the page shows instead of it, and
//! how its latest read went. A diff that is not one Orchester reads is a state of the page, not an error:
//! the pull request itself still shows. Every function blocks on SQLite, blob files or `gh`.

use std::{path::Path, sync::Arc};

use orchester_core::{
  pull_request::{Fields, PullRequest, SyncFailure},
  repository::GitHubRepo,
  revision::Revision,
  store::{PullRequestDiffStore, RepositoryStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::{
  InboxError,
  detail::{Found, FoundDiff, ReadDiff, find_in, refresh_found_in},
  linked::LinkedTask,
  status::SyncStatus,
};
use crate::tasks::changes::{ChangesError, Checkpointed, checkpointed_in};

/// What the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PullRequestPage {
  /// The project has no linked GitHub repository.
  Unlinked,
  Linked(Box<PullRequestView>),
}

/// A pull request of the linked repository as far as it is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullRequestView {
  pub repo: GitHubRepo,
  pub number: u64,
  /// From the latest read, else from the list the last sync stored; `None` when neither has it.
  pub pull_request: Option<PullRequest>,
  pub task: Option<LinkedTask>,
  pub diff: PageDiff,
  /// When what is shown was read from GitHub; `None` when nothing was.
  pub synced_at: Option<Timestamp>,
  /// Why the latest read did not read the pull request.
  pub failure: Option<SyncFailure>,
  /// Why the latest read read the pull request but not its diff; the diff shown is the one stored last.
  pub diff_failure: Option<SyncFailure>,
}

/// The diff the page can show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageDiff {
  /// None is stored and none was read.
  Unknown,
  Read(Arc<ReadDiff>),
  /// The pull request changes more than Orchester reads (300 files, 20,000 lines).
  TooLarge,
  /// What GitHub answered is not a unified diff Orchester reads.
  Unreadable,
}

/// What stands under the page's toolbar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
  /// Nothing of the diff is known: a read is under way, or it failed and the notice says why.
  NotRead,
  TooLarge,
  Unreadable,
  /// The head has no change against the base.
  NoChanges,
  Files(Arc<ReadDiff>),
}

impl PullRequestView {
  /// What the latest read could not read: the pull request, else its diff.
  pub fn latest_failure(&self) -> Option<&SyncFailure> {
    self.failure.as_ref().or(self.diff_failure.as_ref())
  }

  /// How the latest read went, for the context line.
  pub fn status(&self) -> SyncStatus {
    SyncStatus::after(self.synced_at, self.latest_failure())
  }

  pub fn body(&self) -> Body {
    match &self.diff {
      PageDiff::Read(read) if read.diff.files().is_empty() => Body::NoChanges,
      PageDiff::Read(read) => Body::Files(read.clone()),
      PageDiff::TooLarge => Body::TooLarge,
      PageDiff::Unreadable => Body::Unreadable,
      PageDiff::Unknown => Body::NotRead,
    }
  }

  /// How many files the pull request changes: its diff's, else what GitHub counted; `None` before either
  /// is known.
  pub fn file_count(&self) -> Option<usize> {
    match &self.diff {
      PageDiff::Read(read) => Some(read.diff.files().len()),
      PageDiff::Unknown | PageDiff::TooLarge | PageDiff::Unreadable => self
        .pull_request
        .as_ref()
        .and_then(|pull_request| usize::try_from(pull_request.changes().files).ok()),
    }
  }
}

/// Pull request `number` as stored, without asking GitHub.
pub fn load_page_in(repo_root: &Path, number: u64) -> Result<PullRequestPage, InboxError> {
  Ok(page(find_in(repo_root, number)?))
}

/// Reads pull request `number` and the diff of its head from GitHub with `programs`, as the page's Refresh
/// does, unless a rate limit holds syncs back; then what is stored comes back with the rate limit.
pub fn refresh_page_in(repo_root: &Path, number: u64, programs: &Programs) -> Result<PullRequestPage, InboxError> {
  Ok(page(refresh_found_in(repo_root, (number, Fields::Page), programs)?))
}

/// The diff Orchester read for pull request `number` at an earlier head `head`, e.g. the one a feedback thread was
/// written on; `None` when none was read for it, or it is not a unified diff.
pub fn diff_at_in(repo_root: &Path, number: u64, head: &Revision) -> Result<Option<Arc<ReadDiff>>, InboxError> {
  let db = ProjectDb::open(repo_root)?;
  let Some(link) = db.repository_link()? else {
    return Ok(None);
  };
  let Some(stored) = db.pull_request_diff(link.repo(), number, head)? else {
    return Ok(None);
  };
  Ok(match ReadDiff::parse(stored) {
    Ok(read) => Some(Arc::new(read)),
    Err(error) => {
      tracing::warn!(%error, number, "a stored diff of the pull request is not a unified diff");
      None
    }
  })
}

/// What changed from `from` to `to`, commits of the pull request's branch, read with git: what an agent's fix commit
/// changed since the request's revision. `None` when it is larger than Orchester reads.
pub fn between_in(
  repo_root: &Path,
  programs: &Programs,
  (from, to): (&Revision, &Revision),
) -> Result<Option<Arc<ReadDiff>>, ChangesError> {
  Ok(match checkpointed_in(repo_root, programs, from, Some(to))? {
    Checkpointed::Read { changes, .. } => Some(Arc::new(ReadDiff {
      base: from.clone(),
      head: to.clone(),
      synced_at: Timestamp::now(),
      diff: changes.diff.clone(),
    })),
    Checkpointed::TooLarge { .. } | Checkpointed::NoCheckpoint => None,
  })
}

fn page(found: Option<Found>) -> PullRequestPage {
  let Some(found) = found else {
    return PullRequestPage::Unlinked;
  };
  let diff = match found.diff {
    FoundDiff::Unknown => PageDiff::Unknown,
    FoundDiff::TooLarge => PageDiff::TooLarge,
    FoundDiff::Text(text) => match ReadDiff::parse(text) {
      Ok(read) => PageDiff::Read(Arc::new(read)),
      Err(error) => {
        tracing::warn!(%error, number = found.number, "the pull request's diff is not a unified diff");
        PageDiff::Unreadable
      }
    },
  };
  PullRequestPage::Linked(Box::new(PullRequestView {
    repo: found.repo,
    number: found.number,
    pull_request: found.pull_request,
    task: found.task,
    diff,
    synced_at: found.synced_at,
    failure: found.failure,
    diff_failure: found.diff_failure,
  }))
}
