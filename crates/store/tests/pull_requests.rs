//! The cache of a repository's pull requests, and how its latest sync went.

use std::fs;

use orchester_core::{
  connection::GitHubHost,
  diff::Changes,
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  pull_request::{CheckRollup, PullRequest, PullRequestParts, PullRequestState, ReviewDecision},
  repository::GitHubRepo,
  revision::Revision,
  run::BranchName,
  store::PullRequestStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rusqlite::Connection as Sqlite;
use tempfile::TempDir;

fn project_repo() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn repo(name: &str) -> GitHubRepo {
  GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", name).expect("valid repository")
}

fn pull_request(repo: &GitHubRepo, number: u64, updated_at: i64) -> PullRequest {
  let head_sha = Revision::new(&format!("{number:0>40}")).expect("valid revision");
  PullRequest::new(
    repo,
    PullRequestParts {
      number,
      title: format!("Change {number}"),
      url: format!("https://github.com/ivanzaida/{}/pull/{number}", repo.name()),
      state: if number.is_multiple_of(2) {
        PullRequestState::Draft
      } else {
        PullRequestState::Open
      },
      head: BranchName::new(&format!("orc/change-{number}")).expect("valid branch"),
      base: BranchName::new("main").expect("valid branch"),
      checks: CheckRollup {
        passed: 3,
        failed: 1,
        pending: 2,
        skipped: 1,
        on: head_sha.clone(),
      },
      head_sha,
      author: (number != 7).then(|| "ivanzaida".to_owned()),
      changes: Changes {
        files: 6,
        additions: 248,
        deletions: 32,
      },
      review_decision: (number == 42).then_some(ReviewDecision::ChangesRequested),
      updated_at: Timestamp::from_unix_millis(updated_at),
      merged_at: (number == 9).then(|| Timestamp::from_unix_millis(updated_at - 5)),
      closed_at: (number == 9).then(|| Timestamp::from_unix_millis(updated_at - 5)),
      cross_repository: number == 7,
    },
  )
  .expect("valid pull request")
}

#[test]
fn pull_requests_are_unknown_before_the_first_sync() {
  let (_repo, db) = project_repo();
  assert_eq!(db.pull_requests(&repo("kontur")).expect("read"), None);
}

#[test]
fn pull_requests_round_trip_most_recently_updated_first() {
  let (_repo, mut db) = project_repo();
  let kontur = repo("kontur");
  let listed = [
    pull_request(&kontur, 7, 1_000),
    pull_request(&kontur, 42, 3_000),
    pull_request(&kontur, 8, 2_000),
  ];
  db.replace_pull_requests(&kontur, &listed, Timestamp::from_unix_millis(5_000))
    .expect("stored");
  let synced = db.pull_requests(&kontur).expect("read").expect("synced");
  assert_eq!(synced.synced_at, Timestamp::from_unix_millis(5_000));
  assert_eq!(
    synced.pull_requests,
    [listed[1].clone(), listed[2].clone(), listed[0].clone()]
  );
}

#[test]
fn pull_requests_are_replaced_per_repository() {
  let (_repo, mut db) = project_repo();
  let (kontur, lurq) = (repo("kontur"), repo("lurq"));
  db.replace_pull_requests(&kontur, &[pull_request(&kontur, 1, 1)], Timestamp::from_unix_millis(10))
    .expect("stored");
  db.replace_pull_requests(&lurq, &[pull_request(&lurq, 5, 1)], Timestamp::from_unix_millis(20))
    .expect("stored");
  // A later sync without the pull request (merged or closed since) drops it.
  db.replace_pull_requests(&kontur, &[], Timestamp::from_unix_millis(30))
    .expect("stored");
  let kontur_synced = db.pull_requests(&kontur).expect("read").expect("synced");
  assert_eq!(kontur_synced.synced_at, Timestamp::from_unix_millis(30));
  assert!(kontur_synced.pull_requests.is_empty());
  let lurq_synced = db.pull_requests(&lurq).expect("read").expect("synced");
  assert_eq!(lurq_synced.pull_requests, [pull_request(&lurq, 5, 1)]);
}

#[test]
fn damaged_pull_request_rows_are_reported() {
  let (repo_dir, mut db) = project_repo();
  let kontur = repo("kontur");
  db.replace_pull_requests(&kontur, &[pull_request(&kontur, 3, 1)], Timestamp::from_unix_millis(1))
    .expect("stored");
  let raw = Sqlite::open(repo_dir.path().join(".orchester").join("orchester.db")).expect("open raw");
  raw
    .execute(
      "UPDATE pull_requests SET url = 'https://evil.example.com/x/y/pull/3'",
      [],
    )
    .expect("damage the row");
  assert!(matches!(
    db.pull_requests(&kontur),
    Err(StoreError::Corrupt {
      what: "pull request",
      ..
    })
  ));
}

#[test]
fn pull_requests_keep_when_they_were_merged_and_whether_a_fork_opened_them() {
  let (_dir, mut db) = project_repo();
  let kontur = repo("kontur");
  db.replace_pull_requests(
    &kontur,
    &[pull_request(&kontur, 9, 900), pull_request(&kontur, 7, 800)],
    ms(1_000),
  )
  .expect("stored");
  let read = db.pull_requests(&kontur).expect("read").expect("synced").pull_requests;
  assert_eq!(read[0].merged_at(), Some(Timestamp::from_unix_millis(895)));
  assert!(read[1].is_cross_repository());
}

#[test]
fn pull_request_sync_attempts_keep_how_the_latest_went() {
  use orchester_core::{
    connection::{GitHubLogin, GitHubState},
    pull_request::{SyncAttempt, SyncFailure},
    repository::RepositoryError,
  };
  let (_dir, mut db) = project_repo();
  let kontur = repo("kontur");
  assert_eq!(db.pull_request_sync(&kontur).expect("read"), None);
  let login = |name| GitHubLogin::new(name).expect("a login");
  let failures = [
    SyncFailure::RateLimited {
      resets_at: Some(ms(5_000)),
    },
    SyncFailure::Account(GitHubState::WrongAccount {
      active: login("sam-work"),
      expected: login("ivanzaida"),
    }),
    SyncFailure::Account(GitHubState::MissingScope {
      account: login("ivanzaida"),
    }),
    SyncFailure::Repository(RepositoryError::NotFound(kontur.clone())),
    SyncFailure::Repository(RepositoryError::ServerError {
      host: kontur.host().clone(),
      status: Some(502),
      detail: String::new(),
    }),
  ];
  for failure in failures {
    let attempt = SyncAttempt {
      at: ms(2_000),
      failure: Some(failure),
    };
    db.record_pull_request_sync(&kontur, &attempt).expect("recorded");
    assert_eq!(db.pull_request_sync(&kontur).expect("read"), Some(attempt));
  }
  let read = SyncAttempt {
    at: ms(3_000),
    failure: None,
  };
  db.record_pull_request_sync(&kontur, &read).expect("recorded");
  assert_eq!(db.pull_request_sync(&kontur).expect("read"), Some(read));
}

fn ms(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}
