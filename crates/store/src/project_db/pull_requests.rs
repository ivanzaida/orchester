//! The `pull_requests` cache: the open pull requests of a repository as the last sync read
//! them, and `pull_request_syncs`: when that was.

use orchester_core::{
  pull_request::{
    Changes, CheckRollup, PullRequest, PullRequestParts, PullRequestState, ReviewDecision, SyncedPullRequests,
  },
  repository::GitHubRepo,
  revision::Revision,
  run::BranchName,
  store::PullRequestStore,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Row, Transaction, params};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

const COLUMNS: &str = "number, title, url, state, head_branch, base_branch, head_sha, author, changed_files, \
                       additions, deletions, checks_passed, checks_failed, checks_pending, checks_skipped, \
                       review_decision, updated_at";

impl PullRequestStore for ProjectDb {
  type Error = StoreError;

  fn replace_pull_requests(
    &mut self,
    repo: &GitHubRepo,
    pull_requests: &[PullRequest],
    synced_at: Timestamp,
  ) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let key = params![repo.host().as_str(), repo.owner(), repo.name()];
    // Deleting the sync row removes its pull requests (ON DELETE CASCADE).
    transaction.execute(
      "DELETE FROM pull_request_syncs WHERE host = ?1 AND owner = ?2 AND name = ?3",
      key,
    )?;
    transaction.execute(
      "INSERT INTO pull_request_syncs (host, owner, name, synced_at) VALUES (?1, ?2, ?3, ?4)",
      params![repo.host().as_str(), repo.owner(), repo.name(), synced_at.unix_millis()],
    )?;
    for pull_request in pull_requests {
      insert(&transaction, repo, pull_request)?;
    }
    transaction.commit()?;
    Ok(())
  }

  fn pull_requests(&self, repo: &GitHubRepo) -> Result<Option<SyncedPullRequests>, StoreError> {
    let key = params![repo.host().as_str(), repo.owner(), repo.name()];
    let synced_at: Option<i64> = self
      .connection
      .query_row(
        "SELECT synced_at FROM pull_request_syncs WHERE host = ?1 AND owner = ?2 AND name = ?3",
        key,
        |row| row.get(0),
      )
      .optional()?;
    let Some(synced_at) = synced_at else {
      return Ok(None);
    };
    let mut statement = self.connection.prepare(&format!(
      "SELECT {COLUMNS} FROM pull_requests WHERE host = ?1 AND owner = ?2 AND name = ?3 \
       ORDER BY updated_at DESC, number DESC"
    ))?;
    let stored = statement
      .query_map(key, StoredPullRequest::read)?
      .collect::<Result<Vec<_>, _>>()?;
    let pull_requests = stored
      .into_iter()
      .map(|stored| stored.into_pull_request(repo))
      .collect::<Result<_, _>>()?;
    Ok(Some(SyncedPullRequests {
      synced_at: Timestamp::from_unix_millis(synced_at),
      pull_requests,
    }))
  }
}

fn insert(transaction: &Transaction<'_>, repo: &GitHubRepo, pull_request: &PullRequest) -> Result<(), StoreError> {
  let number = i64::try_from(pull_request.number()).map_err(|_| StoreError::Unrepresentable("pull request number"))?;
  let (changes, checks) = (pull_request.changes(), pull_request.checks());
  transaction.execute(
    &format!(
      "INSERT INTO pull_requests (host, owner, name, {COLUMNS}) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)"
    ),
    params![
      repo.host().as_str(),
      repo.owner(),
      repo.name(),
      number,
      pull_request.title(),
      pull_request.url(),
      pull_request.state().as_str(),
      pull_request.head().as_str(),
      pull_request.base().as_str(),
      pull_request.head_sha().as_str(),
      pull_request.author(),
      changes.files,
      changes.additions,
      changes.deletions,
      checks.passed,
      checks.failed,
      checks.pending,
      checks.skipped,
      pull_request.review_decision().map(ReviewDecision::as_str),
      pull_request.updated_at().unix_millis(),
    ],
  )?;
  Ok(())
}

struct StoredPullRequest {
  number: i64,
  title: String,
  url: String,
  state: String,
  head: String,
  base: String,
  head_sha: String,
  author: Option<String>,
  changes: [u32; 3],
  checks: [u32; 4],
  review_decision: Option<String>,
  updated_at: i64,
}

impl StoredPullRequest {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      number: row.get(0)?,
      title: row.get(1)?,
      url: row.get(2)?,
      state: row.get(3)?,
      head: row.get(4)?,
      base: row.get(5)?,
      head_sha: row.get(6)?,
      author: row.get(7)?,
      changes: [row.get(8)?, row.get(9)?, row.get(10)?],
      checks: [row.get(11)?, row.get(12)?, row.get(13)?, row.get(14)?],
      review_decision: row.get(15)?,
      updated_at: row.get(16)?,
    })
  }

  fn into_pull_request(self, repo: &GitHubRepo) -> Result<PullRequest, StoreError> {
    let branch = |what, name: &str| BranchName::new(name).map_err(|error| corrupt(what, error));
    let head_sha = Revision::new(&self.head_sha).map_err(|error| corrupt("pull request head", error))?;
    let review_decision = match self.review_decision.as_deref() {
      None => None,
      Some(value) => Some(ReviewDecision::parse(value).ok_or_else(|| corrupt("pull request review", value))?),
    };
    let [files, additions, deletions] = self.changes;
    let [passed, failed, pending, skipped] = self.checks;
    let parts = PullRequestParts {
      number: u64::try_from(self.number).map_err(|error| corrupt("pull request number", error))?,
      title: self.title,
      url: self.url,
      state: PullRequestState::parse(&self.state).ok_or_else(|| corrupt("pull request state", &self.state))?,
      head: branch("pull request head branch", &self.head)?,
      base: branch("pull request base branch", &self.base)?,
      checks: CheckRollup {
        passed,
        failed,
        pending,
        skipped,
        on: head_sha.clone(),
      },
      head_sha,
      author: self.author,
      changes: Changes {
        files,
        additions,
        deletions,
      },
      review_decision,
      updated_at: Timestamp::from_unix_millis(self.updated_at),
    };
    PullRequest::new(repo, parts).map_err(|error| corrupt("pull request", error))
  }
}
