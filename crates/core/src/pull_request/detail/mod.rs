//! One pull request with its diff (`docs/design/orchester-workspace.md`, "Pull request review",
//! 08.02 `Rrmhf`). Diffs are cached per revision (`docs/architecture/storage.md`): a refresh reads
//! the pull request's current head and base, and fetches the diff only when no diff of that head
//! is stored yet. The merge base of a head and its base branch does not change while the head
//! stays, so the head alone identifies the diff. Orchester only reads.

mod gh;

pub use self::gh::{gh_compare_diff_command, gh_pr_view_command, parse_gh_compare_diff, parse_gh_pr_view};
use super::{Fields, PullRequest, SyncFailure, sync::sync_failure};
use crate::{
  connection::{GitHubCheck, GitHubState},
  diff::RevisionDiff,
  github::GitHub,
  repository::RepositoryLink,
  revision::Revision,
  store::PullRequestDiffStore,
  time::Timestamp,
};

/// A pull request as `gh pr view` read it, with the revision of its base branch its diff starts
/// from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullRequestHead {
  pub pull_request: PullRequest,
  pub base: Revision,
}

/// What a refresh read about the diff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffRead {
  /// The diff of the pull request's current head, from storage or from GitHub.
  Read(RevisionDiff),
  /// The pull request changes more than [`crate::diff::Changes::is_too_large`] allows; nothing was
  /// fetched.
  TooLarge,
  /// The pull request was read, but GitHub gave no diff of its head.
  Failed(SyncFailure),
}

/// The pull request and its diff as one refresh read them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefreshedDetail {
  pub pull_request: PullRequest,
  pub diff: DiffRead,
}

/// The check of `gh` a refresh made, and what it read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailRefresh {
  pub check: GitHubCheck,
  pub outcome: Result<RefreshedDetail, SyncFailure>,
}

/// Reads pull request `number` of the linked repository through `github`, if `gh` acts as the
/// account the link expects, and makes sure `store` holds the diff of its current head: a stored
/// diff of that head is reused (and marked synced at `now`), otherwise it is fetched and stored.
/// Only storage failures are errors; what GitHub refused is the refresh's outcome, and a diff it
/// refused after reading the pull request is [`DiffRead::Failed`].
pub fn refresh_pull_request<S: PullRequestDiffStore>(
  github: &impl GitHub,
  store: &mut S,
  link: &RepositoryLink,
  (number, fields): (u64, Fields),
  now: Timestamp,
) -> Result<DetailRefresh, S::Error> {
  let check = github.check(now);
  let state = GitHubState::of(Some(&check.result), Some(link.expected_login()));
  if !matches!(state, GitHubState::SignedIn { .. }) {
    return Ok(DetailRefresh {
      check,
      outcome: Err(SyncFailure::Account(state)),
    });
  }
  let repo = link.repo();
  let head = match github.pull_request(repo, number, fields) {
    Ok(head) => head,
    Err(error) => {
      return Ok(DetailRefresh {
        check,
        outcome: Err(sync_failure(github, repo, error)),
      });
    }
  };
  let PullRequestHead { pull_request, base } = head;
  if pull_request.changes().is_too_large() {
    let outcome = Ok(RefreshedDetail {
      pull_request,
      diff: DiffRead::TooLarge,
    });
    return Ok(DetailRefresh { check, outcome });
  }
  let head_sha = pull_request.head_sha().clone();
  // A stored diff keeps the base it was read against: the text is the same for the new one.
  let (base, text) = match store.pull_request_diff(repo, number, &head_sha)? {
    Some(stored) => (stored.base, stored.text),
    None => match github.compare_diff(repo, &base, &head_sha) {
      Ok(text) => (base, text),
      Err(error) => {
        let outcome = Ok(RefreshedDetail {
          pull_request,
          diff: DiffRead::Failed(sync_failure(github, repo, error)),
        });
        return Ok(DetailRefresh { check, outcome });
      }
    },
  };
  let diff = RevisionDiff {
    base,
    head: head_sha,
    text,
    read_at: now,
  };
  store.save_pull_request_diff(repo, number, &diff)?;
  let outcome = Ok(RefreshedDetail {
    pull_request,
    diff: DiffRead::Read(diff),
  });
  Ok(DetailRefresh { check, outcome })
}
