//! The pull request list: `gh pr list` output captured from gh 2.97.0 against `ivanzaida/
//! orchester` (no open pull requests) and `ivanzaida/lurq` (`--state all`, merged pull requests
//! with passing checks), a synthetic list for open, draft, failing, pending and skipped checks
//! and review decisions, and one sync as the expected account.

use orchester_core::{
  command::CommandError,
  connection::{GitHubHost, GitHubLogin, GitHubState},
  github::GhCli,
  id::ConnectionId,
  pull_request::{
    Changes, CheckRollup, PullRequest, PullRequestError, PullRequestParts, PullRequestState, ReviewDecision,
    SyncFailure, gh_pr_list_command, parse_gh_pr_list, parse_gh_rate_limit, sort_by_update, sync_pull_requests,
  },
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryError, RepositoryLink, RepositoryView},
  revision::Revision,
  run::BranchName,
  time::Timestamp,
};

use crate::{
  connection::github_connection,
  replay::{Replay, failed, fixture, gh, now, ok},
};

const VERSION: &str = "--version";
const AUTH_STATUS: &str = "auth status --hostname github.com --json hosts";
const LIST_KONTUR: &str = "pr list -R github.com/ivanzaida/kontur --state open --limit 100 --json \
                           number,title,url,state,isDraft,headRefName,baseRefName,headRefOid,author,additions,\
                           deletions,changedFiles,updatedAt,reviewDecision,statusCheckRollup";
const RATE_LIMIT: &str = "api --hostname github.com rate_limit";

fn repo(owner: &str, name: &str) -> GitHubRepo {
  GitHubRepo::new(GitHubHost::github_com(), owner, name).expect("valid repository")
}

fn login(value: &str) -> GitHubLogin {
  GitHubLogin::new(value).expect("valid login")
}

fn link(expected: &str) -> RepositoryLink {
  let view = RepositoryView {
    repo: repo("ivanzaida", "kontur"),
    default_branch: BranchName::new("main").expect("valid branch"),
    permission: RepoPermission::Admin,
  };
  RepositoryLink::new(
    ConnectionId::generate(),
    RemoteName::origin(),
    view,
    login(expected),
    now(),
  )
}

fn synthetic() -> Vec<PullRequest> {
  parse_gh_pr_list(
    &repo("ivanzaida", "kontur"),
    &ok(fixture!("gh_pr_list_synthetic_open.stdout")),
  )
  .expect("a readable list")
}

#[test]
fn pr_list_asks_for_open_pull_requests_of_one_repository() {
  let command = gh_pr_list_command(&gh(), &repo("ivanzaida", "kontur"), 100);
  assert_eq!(command.args().join(" "), LIST_KONTUR);
  assert!(
    command
      .env()
      .iter()
      .any(|(key, value)| key == "GH_PROMPT_DISABLED" && value == "1")
  );
}

#[test]
fn pr_list_of_a_repository_without_open_pull_requests_is_empty() {
  let listed = parse_gh_pr_list(
    &repo("ivanzaida", "orchester"),
    &ok(fixture!("gh_pr_list_orchester_open.stdout")),
  )
  .expect("a readable list");
  assert!(listed.is_empty());
}

#[test]
fn pr_list_reads_captured_merged_pull_requests() {
  let listed =
    parse_gh_pr_list(&repo("ivanzaida", "lurq"), &ok(fixture!("gh_pr_list_lurq_all.stdout"))).expect("a readable list");
  assert_eq!(listed.len(), 10);
  let latest = &listed[0];
  assert_eq!(latest.number(), 28);
  assert_eq!(latest.title(), "chore(release): 0.23.0");
  assert_eq!(latest.url(), "https://github.com/ivanzaida/lurq/pull/28");
  assert_eq!(latest.state(), PullRequestState::Merged);
  assert_eq!(latest.head().as_str(), "codex/release-0.23.0");
  assert_eq!(latest.base().as_str(), "master");
  assert_eq!(latest.head_sha().as_str(), "494513deeacd8dc8be7d6501500a2d0faf2ff4b9");
  assert_eq!(latest.author(), Some("ivanzaida"));
  assert_eq!(
    latest.changes(),
    Changes {
      files: 17,
      additions: 19,
      deletions: 19
    }
  );
  assert_eq!((latest.checks().passed, latest.checks().total()), (3, 3));
  assert_eq!(&latest.checks().on, latest.head_sha());
  assert_eq!(latest.review_decision(), None);
  assert_eq!(
    latest.updated_at(),
    Timestamp::parse_utc("2026-09-24T12:38:56Z").expect("a UTC time")
  );
}

#[test]
fn pr_list_reads_states_checks_and_reviews() {
  let listed = synthetic();
  let [auth, switcher, docs] = listed.as_slice() else {
    panic!("three pull requests");
  };
  assert_eq!(auth.state(), PullRequestState::Open);
  assert_eq!(auth.review_decision(), Some(ReviewDecision::ReviewRequired));
  // A success, a neutral check run and a successful commit status.
  assert_eq!((auth.checks().passed, auth.checks().total()), (3, 3));

  assert_eq!(switcher.state(), PullRequestState::Draft);
  let checks = switcher.checks();
  assert_eq!(
    (checks.passed, checks.failed, checks.pending, checks.skipped),
    (0, 1, 2, 1)
  );

  assert_eq!(docs.review_decision(), Some(ReviewDecision::ChangesRequested));
  assert_eq!(docs.author(), Some("app/dependabot"));
  assert_eq!(docs.checks().total(), 0);
}

#[test]
fn pr_list_failures_are_classified() {
  let requested = repo("ivanzaida", "does-not-exist-orch-xyz");
  assert_eq!(
    parse_gh_pr_list(&requested, &failed(fixture!("gh_pr_list_not_found.stderr"))),
    Err(RepositoryError::NotFound(requested.clone()))
  );
  // Synthetic: the message gh prints for an exhausted GraphQL rate limit.
  assert_eq!(
    parse_gh_pr_list(&requested, &failed(fixture!("gh_repo_view_rate_limited.stderr"))),
    Err(RepositoryError::RateLimited)
  );
  assert!(matches!(
    parse_gh_pr_list(&requested, &ok("{\"not\":\"a list\"}")),
    Err(RepositoryError::Unrecognized { program: "gh", .. })
  ));
}

#[test]
fn pr_list_rejects_unknown_check_outcomes() {
  let unknown = fixture!("gh_pr_list_synthetic_open.stdout").replace("\"NEUTRAL\"", "\"SOMETHING_NEW\"");
  assert!(matches!(
    parse_gh_pr_list(&repo("ivanzaida", "kontur"), &ok(&unknown)),
    Err(RepositoryError::Unrecognized { .. })
  ));
}

#[test]
fn rate_limit_reset_is_the_graphql_reset() {
  let reset =
    parse_gh_rate_limit(&repo("ivanzaida", "kontur"), &ok(fixture!("gh_api_rate_limit.stdout"))).expect("a reset time");
  assert_eq!(reset, Timestamp::from_unix_seconds(1_790_355_635));
}

fn parts(url: &str) -> PullRequestParts {
  let head_sha = Revision::new("8f3a2c1d4e5f60718293a4b5c6d7e8f901234567").expect("valid revision");
  PullRequestParts {
    number: 42,
    title: "Add GitHub authentication".to_owned(),
    url: url.to_owned(),
    state: PullRequestState::Open,
    head: BranchName::new("orc/github-auth").expect("valid branch"),
    base: BranchName::new("main").expect("valid branch"),
    checks: CheckRollup::none(head_sha.clone()),
    head_sha,
    author: Some("ivanzaida".to_owned()),
    changes: Changes::default(),
    review_decision: None,
    updated_at: now(),
  }
}

#[test]
fn pull_request_addresses_are_checked() {
  let kontur = repo("ivanzaida", "kontur");
  assert!(PullRequest::new(&kontur, parts("https://github.com/ivanzaida/kontur/pull/42")).is_ok());
  for url in [
    "https://token@github.com/ivanzaida/kontur/pull/42",
    "http://github.com/ivanzaida/kontur/pull/42",
    "https://github.example.com/ivanzaida/kontur/pull/42",
    "https://github.com/ivanzaida/kontur/pull/43",
    "https://github.com/ivanzaida/kontur/pull/42?x=1",
    "file:///C:/Windows/system32/calc.exe",
  ] {
    assert_eq!(
      PullRequest::new(&kontur, parts(url)),
      Err(PullRequestError::Url(url.to_owned())),
      "{url}"
    );
  }
}

#[test]
fn pull_requests_are_found_by_number_title_and_branch() {
  let listed = synthetic();
  let found = |query: &str| {
    listed
      .iter()
      .filter(|pull_request| pull_request.matches(query))
      .map(PullRequest::number)
      .collect::<Vec<_>>()
  };
  assert_eq!(found(""), [42, 43, 44]);
  assert_eq!(found("#43"), [43]);
  assert_eq!(found("44"), [44]);
  assert_eq!(found("github AUTH"), [42]);
  assert_eq!(found("docs/"), [44]);
  assert_eq!(found("nothing like this"), Vec::<u64>::new());
}

#[test]
fn pull_requests_sort_most_recently_updated_first() {
  let mut listed = synthetic();
  listed.reverse();
  sort_by_update(&mut listed);
  assert_eq!(listed.iter().map(PullRequest::number).collect::<Vec<_>>(), [42, 43, 44]);
}

#[test]
fn utc_times_are_read_as_github_writes_them() {
  assert_eq!(
    Timestamp::parse_utc("1970-01-01T00:00:00Z"),
    Some(Timestamp::from_unix_millis(0))
  );
  assert_eq!(
    Timestamp::parse_utc("2026-09-24T12:38:56Z"),
    Some(Timestamp::from_unix_seconds(1_790_253_536))
  );
  assert_eq!(
    Timestamp::parse_utc("2024-02-29T23:59:59.5Z"),
    Some(Timestamp::from_unix_millis(1_709_251_199_500))
  );
  for invalid in [
    "2026-09-24T12:38:56",
    "2026-02-30T00:00:00Z",
    "2026-09-24 12:38:56Z",
    "yesterday",
  ] {
    assert_eq!(Timestamp::parse_utc(invalid), None, "{invalid}");
  }
}

fn sync_with(
  answers: Vec<(
    &'static str,
    Result<orchester_core::command::CommandOutput, CommandError>,
  )>,
  expected: &str,
) -> (Replay, orchester_core::pull_request::SyncResult) {
  let runner = Replay::new(answers);
  let connection = github_connection();
  let result = sync_pull_requests(&GhCli::new(&runner, &connection), &link(expected), now());
  (runner, result)
}

fn signed_in() -> Vec<(
  &'static str,
  Result<orchester_core::command::CommandOutput, CommandError>,
)> {
  vec![
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(ok(fixture!("gh_auth_status_signed_in.stdout")))),
  ]
}

#[test]
fn sync_reads_the_list_as_the_expected_account() {
  let mut answers = signed_in();
  answers.push((LIST_KONTUR, Ok(ok(fixture!("gh_pr_list_synthetic_open.stdout")))));
  let (_, result) = sync_with(answers, "ivanzaida");
  let listed = result.outcome.expect("the list");
  assert_eq!(listed.len(), 3);
  assert_eq!(result.check.result.signed_in_account(), Some(&login("ivanzaida")));
}

#[test]
fn sync_is_paused_for_another_account() {
  let (runner, result) = sync_with(signed_in(), "ivan-work");
  assert_eq!(
    result.outcome,
    Err(SyncFailure::Account(GitHubState::WrongAccount {
      active: login("ivanzaida"),
      expected: login("ivan-work"),
    }))
  );
  // Only the check ran: nothing was read as the wrong account.
  assert!(runner.ran().iter().all(|command| command.args()[0] != "pr"));
}

#[test]
fn sync_reports_when_the_rate_limit_resets() {
  let mut answers = signed_in();
  answers.push((LIST_KONTUR, Ok(failed(fixture!("gh_repo_view_rate_limited.stderr")))));
  answers.push((RATE_LIMIT, Ok(ok(fixture!("gh_api_rate_limit.stdout")))));
  let (_, result) = sync_with(answers, "ivanzaida");
  assert_eq!(
    result.outcome,
    Err(SyncFailure::RateLimited {
      resets_at: Some(Timestamp::from_unix_seconds(1_790_355_635))
    })
  );

  let mut answers = signed_in();
  answers.push((LIST_KONTUR, Ok(failed(fixture!("gh_repo_view_rate_limited.stderr")))));
  answers.push((RATE_LIMIT, Err(CommandError::TimedOut)));
  let (_, result) = sync_with(answers, "ivanzaida");
  assert_eq!(result.outcome, Err(SyncFailure::RateLimited { resets_at: None }));
}

#[test]
fn sync_keeps_network_failures_typed() {
  let mut answers = signed_in();
  answers.push((LIST_KONTUR, Ok(failed(fixture!("gh_repo_view_network.stderr")))));
  let (_, result) = sync_with(answers, "ivanzaida");
  assert!(matches!(
    result.outcome,
    Err(SyncFailure::Repository(RepositoryError::Network { .. }))
  ));
}
