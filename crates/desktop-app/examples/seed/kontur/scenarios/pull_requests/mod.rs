//! `pull-requests`: Kontur linked to `ivanzaida/kontur` through a GitHub connection whose program is a fake
//! `gh` ([`gh`]) answering from files, with the pull requests of the design's inbox (`Pull requests` `l0YNS`):
//! six open ones, three of them from the branches of running runs, one from a run that succeeded, a draft
//! and one no run made; three merged and one closed. The list is stored as a sync a minute ago left it, and
//! the fake answers a sync with the same list. Which answer it gives is the one word in
//! `.orchester/qa-bin/gh-mode.txt` (`ok`, `wrong-account`, `missing-scope`, `not-signed-in`, `unreachable`,
//! `server-error`, `rate-limited`), so QA reaches a state by writing the word and pressing Refresh; removing
//! the program reaches *gh not found*.

mod gh;

use PullRequestState::{Closed, Draft, Merged, Open};
use orchester_core::{
  connection::{GitHubHost, GitHubLogin},
  diff::Changes,
  pull_request::{CheckRollup, PullRequest, PullRequestParts, PullRequestState, ReviewDecision},
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryLink, RepositoryView},
  revision::Revision,
  run::BranchName,
  store::{PullRequestStore, RepositoryStore, RunStore},
  time::Timestamp,
};

use crate::kontur::{
  Kontur, SeedError,
  shapes::{self, Shape},
};

/// The open, draft and closed pull requests, most recently updated first: number, title, the task's shape
/// whose run's branch it is from (or the branch no run made), state, review, checks (passed, running,
/// failed), the changes, and how many minutes ago it was updated.
const PULL_REQUESTS: [Listed; 10] = [
  listed(
    142,
    "Add retry to webhook sender",
    From::Task(Shape::Running("implement")),
    Open,
    Some(ReviewDecision::ReviewRequired),
    (5, 1, 0),
    (6, 133, 15),
    8,
  ),
  listed(
    137,
    "Typed errors per module",
    From::Task(Shape::Succeeded),
    Open,
    Some(ReviewDecision::ChangesRequested),
    (4, 0, 1),
    (14, 412, 198),
    21,
  ),
  listed(
    139,
    "Rate-limit messages from GitHub",
    From::Task(Shape::Running("implement")),
    Draft,
    None,
    (2, 0, 0),
    (3, 41, 6),
    60,
  ),
  listed(
    136,
    "Explain an expired GitHub token in the middle of a long review step and how to sign in again from the status bar",
    From::Task(Shape::Running("review")),
    Open,
    Some(ReviewDecision::ReviewRequired),
    (3, 0, 0),
    (4, 97, 8),
    3 * 60,
  ),
  listed(
    134,
    "Bump tokio to 1.48",
    From::Branch("deps/tokio-1.48"),
    Open,
    Some(ReviewDecision::Approved),
    (6, 0, 0),
    (2, 3, 3),
    24 * 60,
  ),
  listed(
    133,
    "Import contacts from CSV",
    From::Task(Shape::Succeeded),
    Open,
    None,
    (0, 0, 0),
    (5, 188, 4),
    2 * 24 * 60,
  ),
  listed(
    131,
    "Show the signed-in account",
    From::Task(Shape::Merged),
    Merged,
    Some(ReviewDecision::Approved),
    (6, 0, 0),
    (4, 96, 11),
    2 * 24 * 60 + 60,
  ),
  listed(
    128,
    "Stream run events to the status bar",
    From::Task(Shape::Merged),
    Merged,
    Some(ReviewDecision::Approved),
    (5, 0, 0),
    (7, 214, 40),
    4 * 24 * 60,
  ),
  listed(
    127,
    "Update the README badges",
    From::Branch("readme-badges"),
    Merged,
    None,
    (0, 0, 0),
    (1, 4, 2),
    5 * 24 * 60,
  ),
  listed(
    125,
    "Try a GraphQL client for the sync",
    From::Branch("spike/graphql-client"),
    Closed,
    None,
    (1, 0, 1),
    (9, 320, 12),
    6 * 24 * 60,
  ),
];

/// Where a seeded pull request's branch comes from.
#[derive(Clone, Copy)]
enum From {
  /// The run of a task titled as the pull request, left in this shape.
  Task(Shape),
  Branch(&'static str),
}

#[derive(Clone, Copy)]
struct Listed {
  number: u64,
  title: &'static str,
  from: From,
  state: PullRequestState,
  review: Option<ReviewDecision>,
  checks: (u32, u32, u32),
  changes: (u32, u32, u32),
  updated_minutes_ago: i64,
}

#[allow(clippy::too_many_arguments, reason = "one table row of the design's inbox")]
const fn listed(
  number: u64,
  title: &'static str,
  from: From,
  state: PullRequestState,
  review: Option<ReviewDecision>,
  checks: (u32, u32, u32),
  changes: (u32, u32, u32),
  updated_minutes_ago: i64,
) -> Listed {
  Listed {
    number,
    title,
    from,
    state,
    review,
    checks,
    changes,
    updated_minutes_ago,
  }
}

const BRIEF: &str = "Deliver the change behind the pull request.";
const CRITERIA: [&str; 1] = ["The change is covered by tests."];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let mut pull_requests = Vec::new();
  for (index, listed) in PULL_REQUESTS.into_iter().enumerate() {
    let head = match listed.from {
      From::Branch(branch) => branch_name(branch)?,
      From::Task(shape) => {
        let started = kontur.ago(listed.updated_minutes_ago + 90 + i64::try_from(index).expect("a short list"));
        let created = Timestamp::from_unix_millis(started.unix_millis() - 30 * 60_000);
        let mut task = kontur.task(
          (listed.title, BRIEF, &CRITERIA),
          Some(&catalogue.feature_delivery),
          created,
        )?;
        shapes::apply(kontur, &mut task, Some(&catalogue.feature_delivery), shape, started)?;
        let run = kontur
          .db
          .runs_of_task(task.id())?
          .pop()
          .ok_or(SeedError::NotAtWork)?
          .map_err(|unreadable| unreadable.error)?;
        run.branch().clone()
      }
    };
    pull_requests.push(pull_request(kontur, &listed, head)?);
  }
  let repo = repo()?;
  let program = gh::install(kontur, &pull_requests)?;
  let connection = super::connections::github(kontur, program, kontur.ago(60))?;
  let view = RepositoryView {
    repo: repo.clone(),
    default_branch: branch_name("main")?,
    permission: RepoPermission::Admin,
  };
  let link = RepositoryLink::new(
    connection,
    RemoteName::origin(),
    view,
    login("ivanzaida")?,
    kontur.ago(60),
  );
  kontur.db.save_repository_link(&link)?;
  kontur.db.replace_pull_requests(&repo, &pull_requests, kontur.ago(1))?;
  Ok(())
}

fn pull_request(kontur: &Kontur, listed: &Listed, head: BranchName) -> Result<PullRequest, SeedError> {
  let head_sha = revision(listed.number)?;
  let (passed, pending, failed) = listed.checks;
  let (files, additions, deletions) = listed.changes;
  let parts = PullRequestParts {
    number: listed.number,
    title: listed.title.to_owned(),
    url: format!("https://github.com/ivanzaida/kontur/pull/{}", listed.number),
    state: listed.state,
    head,
    base: branch_name("main")?,
    checks: CheckRollup {
      passed,
      failed,
      pending,
      skipped: 0,
      on: head_sha.clone(),
    },
    head_sha,
    author: Some("ivanzaida".to_owned()),
    changes: Changes {
      files,
      additions,
      deletions,
    },
    review_decision: listed.review,
    updated_at: kontur.ago(listed.updated_minutes_ago),
  };
  PullRequest::new(&repo()?, parts).map_err(|error| SeedError::Invalid(error.to_string()))
}

/// A head revision made up from the pull request's number.
fn revision(number: u64) -> Result<Revision, SeedError> {
  let hex = format!("{:x}", number * 0x9e37_79b9);
  Revision::new(&format!("{hex:0>40}")).map_err(|error| SeedError::Invalid(error.to_string()))
}

fn repo() -> Result<GitHubRepo, SeedError> {
  GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", "kontur")
    .map_err(|error| SeedError::Invalid(error.to_string()))
}

fn branch_name(name: &str) -> Result<BranchName, SeedError> {
  BranchName::new(name).map_err(|error| SeedError::Invalid(error.to_string()))
}

fn login(name: &str) -> Result<GitHubLogin, SeedError> {
  GitHubLogin::new(name).map_err(|error| SeedError::Invalid(error.to_string()))
}
