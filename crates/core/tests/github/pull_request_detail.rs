//! A pull request's detail: `gh pr view` and the pinned compare diff captured from gh 2.97.0 for
//! `dtolnay/anyhow#463` (open) and `ivanzaida/lurq#5` (merged), a pull request and a revision that
//! do not exist, and a refresh that reuses the diff stored for an unchanged head.

use std::{cell::RefCell, collections::HashMap, convert::Infallible};

use orchester_core::{
  command::CommandOutput,
  connection::{GitHubHost, GitHubLogin, GitHubState},
  diff::{Changes, RevisionDiff},
  github::GhCli,
  id::ConnectionId,
  pull_request::{
    DiffRead, PullRequestState, SyncFailure, gh_compare_diff_command, gh_pr_view_command, parse_gh_compare_diff,
    parse_gh_pr_view, refresh_pull_request,
  },
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryError, RepositoryLink, RepositoryView},
  revision::Revision,
  run::BranchName,
  store::PullRequestDiffStore,
  time::Timestamp,
};

use crate::{
  connection::github_connection,
  replay::{Replay, failed, fixture, gh, now, ok},
};

const VERSION: &str = "--version";
const AUTH_STATUS: &str = "auth status --hostname github.com --json hosts";
const VIEW_463: &str = "pr view 463 -R github.com/dtolnay/anyhow --json \
                        number,title,url,state,isDraft,headRefName,baseRefName,headRefOid,author,additions,\
                        deletions,changedFiles,updatedAt,reviewDecision,statusCheckRollup,baseRefOid";
const COMPARE_463: &str = "api --allow-escape-sequences --hostname github.com -H Accept: application/vnd.github.diff \
                           repos/dtolnay/anyhow/compare/c63b279f3f4af2b02ca6267d9eb47d6d10497f69...\
                           c7b210a78b32ea90b10860c58983f8c0a742ed03";
const BASE_463: &str = "c63b279f3f4af2b02ca6267d9eb47d6d10497f69";
const HEAD_463: &str = "c7b210a78b32ea90b10860c58983f8c0a742ed03";

fn repo(owner: &str, name: &str) -> GitHubRepo {
  GitHubRepo::new(GitHubHost::github_com(), owner, name).expect("valid repository")
}

fn revision(sha: &str) -> Revision {
  Revision::new(sha).expect("a full revision")
}

fn anyhow_link(expected: &str) -> RepositoryLink {
  let view = RepositoryView {
    repo: repo("dtolnay", "anyhow"),
    default_branch: BranchName::new("master").expect("valid branch"),
    permission: RepoPermission::Read,
  };
  RepositoryLink::new(
    ConnectionId::generate(),
    RemoteName::origin(),
    view,
    GitHubLogin::new(expected).expect("valid login"),
    now(),
  )
}

#[test]
fn view_asks_for_one_pull_request_with_its_base_revision() {
  let command = gh_pr_view_command(&gh(), &repo("dtolnay", "anyhow"), 463);
  assert_eq!(command.args().join(" "), VIEW_463);
}

#[test]
fn compare_asks_for_the_diff_between_exact_revisions() {
  let command = gh_compare_diff_command(
    &gh(),
    &repo("dtolnay", "anyhow"),
    &revision(BASE_463),
    &revision(HEAD_463),
  );
  assert_eq!(command.args().join(" "), COMPARE_463);
  assert!(
    command
      .env()
      .iter()
      .any(|(key, value)| key == "GH_PROMPT_DISABLED" && value == "1")
  );
}

#[test]
fn view_of_an_open_pull_request_gives_its_head_and_base() {
  let head = parse_gh_pr_view(
    &repo("dtolnay", "anyhow"),
    463,
    &ok(fixture!("gh_pr_view_anyhow_463.stdout")),
  )
  .expect("a readable pull request");
  assert_eq!(head.base, revision(BASE_463));
  assert_eq!(head.pull_request.head_sha(), &revision(HEAD_463));
  assert_eq!(head.pull_request.state(), PullRequestState::Open);
  assert_eq!(head.pull_request.head().as_str(), "fix/462-new-with-backtrace");
  assert_eq!(
    head.pull_request.changes(),
    Changes {
      files: 2,
      additions: 29,
      deletions: 0
    }
  );
}

#[test]
fn view_of_a_merged_pull_request_is_readable() {
  let head = parse_gh_pr_view(&repo("ivanzaida", "lurq"), 5, &ok(fixture!("gh_pr_view_lurq_5.stdout")))
    .expect("a readable pull request");
  assert_eq!(head.pull_request.state(), PullRequestState::Merged);
  assert_eq!(head.base, revision("c42ef9bba20874faf35f3cbd77323eb83349bd38"));
}

#[test]
fn view_of_another_number_is_not_accepted() {
  let result = parse_gh_pr_view(
    &repo("dtolnay", "anyhow"),
    462,
    &ok(fixture!("gh_pr_view_anyhow_463.stdout")),
  );
  assert!(matches!(result, Err(RepositoryError::Unrecognized { .. })));
}

#[test]
fn missing_pull_request_and_missing_revisions_are_not_found() {
  let anyhow = repo("dtolnay", "anyhow");
  assert_eq!(
    parse_gh_pr_view(&anyhow, 99_999, &failed(fixture!("gh_pr_view_not_found.stderr"))),
    Err(RepositoryError::PullRequestNotFound {
      repo: anyhow.clone(),
      number: 99_999
    })
  );
  assert_eq!(
    parse_gh_compare_diff(&anyhow, &failed(fixture!("gh_api_compare_not_found.stderr"))),
    Err(RepositoryError::DiffNotFound(anyhow.clone()))
  );
}

#[test]
fn changes_beyond_the_limits_are_too_large_to_read() {
  let changes = |files, additions, deletions| Changes {
    files,
    additions,
    deletions,
  };
  assert!(!changes(2, 29, 0).is_too_large());
  assert!(!changes(300, 10_000, 10_000).is_too_large());
  assert!(changes(301, 1, 0).is_too_large());
  assert!(changes(12, 15_000, 5_001).is_too_large());
}

/// Diffs by (number, head), in memory.
#[derive(Default)]
struct Diffs(RefCell<HashMap<(u64, String), RevisionDiff>>);

impl PullRequestDiffStore for Diffs {
  type Error = Infallible;

  fn save_pull_request_diff(&mut self, _: &GitHubRepo, number: u64, diff: &RevisionDiff) -> Result<(), Infallible> {
    self
      .0
      .borrow_mut()
      .insert((number, diff.head.to_string()), diff.clone());
    Ok(())
  }

  fn pull_request_diff(
    &self,
    _: &GitHubRepo,
    number: u64,
    head: &Revision,
  ) -> Result<Option<RevisionDiff>, Infallible> {
    Ok(self.0.borrow().get(&(number, head.to_string())).cloned())
  }

  fn latest_pull_request_diff(&self, _: &GitHubRepo, number: u64) -> Result<Option<RevisionDiff>, Infallible> {
    let diffs = self.0.borrow();
    let latest = diffs
      .iter()
      .filter(|((stored, _), _)| *stored == number)
      .max_by_key(|(_, diff)| diff.read_at);
    Ok(latest.map(|(_, diff)| diff.clone()))
  }
}

fn signed_in(extra: Vec<(&'static str, CommandOutput)>) -> Replay {
  let mut answers = vec![
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(ok(fixture!("gh_auth_status_signed_in.stdout")))),
  ];
  answers.extend(extra.into_iter().map(|(args, output)| (args, Ok(output))));
  Replay::new(answers)
}

fn ran_compare(runner: &Replay) -> bool {
  runner.ran().iter().any(|command| command.args()[0] == "api")
}

#[test]
fn refresh_fetches_and_stores_the_diff_of_a_new_head() {
  let runner = signed_in(vec![
    (VIEW_463, ok(fixture!("gh_pr_view_anyhow_463.stdout"))),
    (COMPARE_463, ok(fixture!("gh_api_compare_anyhow_463.stdout"))),
  ]);
  let mut store = Diffs::default();
  let connection = github_connection();
  let github = GhCli::new(&runner, &connection);
  let refresh =
    refresh_pull_request(&github, &mut store, &anyhow_link("ivanzaida"), 463, now()).expect("infallible store");
  let detail = refresh.outcome.expect("the pull request");
  let DiffRead::Read(diff) = detail.diff else {
    panic!("the diff was read");
  };
  assert_eq!((diff.base, diff.head.clone()), (revision(BASE_463), revision(HEAD_463)));
  assert_eq!(diff.text, fixture!("gh_api_compare_anyhow_463.stdout"));
  assert!(ran_compare(&runner));
  let stored = store.pull_request_diff(&repo("dtolnay", "anyhow"), 463, &diff.head);
  assert!(stored.expect("infallible store").is_some());
}

#[test]
fn refresh_reuses_the_stored_diff_of_an_unchanged_head() {
  let runner = signed_in(vec![(VIEW_463, ok(fixture!("gh_pr_view_anyhow_463.stdout")))]);
  let mut store = Diffs::default();
  let earlier = RevisionDiff {
    base: revision(BASE_463),
    head: revision(HEAD_463),
    text: fixture!("gh_api_compare_anyhow_463.stdout").to_owned(),
    read_at: Timestamp::from_unix_millis(1),
  };
  let anyhow = repo("dtolnay", "anyhow");
  store
    .save_pull_request_diff(&anyhow, 463, &earlier)
    .expect("infallible store");
  let connection = github_connection();
  let refresh = refresh_pull_request(
    &GhCli::new(&runner, &connection),
    &mut store,
    &anyhow_link("ivanzaida"),
    463,
    now(),
  )
  .expect("infallible store");
  assert!(!ran_compare(&runner));
  let detail = refresh.outcome.expect("the pull request");
  assert_eq!(
    detail.diff,
    DiffRead::Read(RevisionDiff {
      read_at: now(),
      ..earlier
    })
  );
  let latest = store.latest_pull_request_diff(&anyhow, 463).expect("infallible store");
  assert_eq!(latest.map(|diff| diff.read_at), Some(now()));
}

#[test]
fn refresh_as_another_account_reads_nothing() {
  let runner = signed_in(Vec::new());
  let connection = github_connection();
  let refresh = refresh_pull_request(
    &GhCli::new(&runner, &connection),
    &mut Diffs::default(),
    &anyhow_link("ivan-work"),
    463,
    now(),
  )
  .expect("infallible store");
  assert!(matches!(
    refresh.outcome,
    Err(SyncFailure::Account(GitHubState::WrongAccount { .. }))
  ));
  assert!(
    runner
      .ran()
      .iter()
      .all(|command| command.args()[0] == "--version" || command.args()[0] == "auth")
  );
}

#[test]
fn refresh_of_a_missing_pull_request_says_so() {
  let runner = signed_in(vec![(VIEW_463, failed(fixture!("gh_pr_view_not_found.stderr")))]);
  let connection = github_connection();
  let refresh = refresh_pull_request(
    &GhCli::new(&runner, &connection),
    &mut Diffs::default(),
    &anyhow_link("ivanzaida"),
    463,
    now(),
  )
  .expect("infallible store");
  assert_eq!(
    refresh.outcome,
    Err(SyncFailure::Repository(RepositoryError::PullRequestNotFound {
      repo: repo("dtolnay", "anyhow"),
      number: 463
    }))
  );
  assert!(!ran_compare(&runner));
}

#[test]
fn refresh_keeps_the_pull_request_when_github_has_no_diff_of_its_head() {
  let runner = signed_in(vec![
    (VIEW_463, ok(fixture!("gh_pr_view_anyhow_463.stdout"))),
    (COMPARE_463, failed(fixture!("gh_api_compare_not_found.stderr"))),
  ]);
  let mut store = Diffs::default();
  let connection = github_connection();
  let refresh = refresh_pull_request(
    &GhCli::new(&runner, &connection),
    &mut store,
    &anyhow_link("ivanzaida"),
    463,
    now(),
  )
  .expect("infallible store");
  let detail = refresh.outcome.expect("the pull request was read");
  assert_eq!(detail.pull_request.number(), 463);
  assert_eq!(
    detail.diff,
    DiffRead::Failed(SyncFailure::Repository(RepositoryError::DiffNotFound(repo(
      "dtolnay", "anyhow"
    ))))
  );
  let latest = store.latest_pull_request_diff(&repo("dtolnay", "anyhow"), 463);
  assert_eq!(latest.expect("infallible store"), None);
}
