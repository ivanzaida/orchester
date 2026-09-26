//! The pull request list: `gh pr list` output captured from gh 2.97.0 against `ivanzaida/
//! orchester` (no open pull requests) and `ivanzaida/lurq` (`--state all`, merged pull requests
//! with passing checks), a synthetic list for open, draft, failing, pending and skipped checks
//! and review decisions, and one sync as the expected account. `microsoft/vscode`'s 50 most
//! recent open pull requests and the check counts of their heads (captured 2026-09-25) show the
//! two requests a sync makes; asking `gh pr list` for 100 with their checks timed out (HTTP 504).

use std::sync::OnceLock;

use orchester_core::{
  command::CommandError,
  connection::{GitHubHost, GitHubLogin, GitHubState},
  github::GhCli,
  id::ConnectionId,
  pull_request::{
    Changes, CheckRollup, PullRequest, PullRequestError, PullRequestParts, PullRequestState, ReviewDecision,
    SYNC_LIMIT, SyncFailure, gh_check_rollups_command, gh_pr_list_command, parse_gh_check_rollups, parse_gh_pr_list,
    parse_gh_rate_limit, sort_by_update, sync_pull_requests,
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
const LIST_KONTUR: &str = "pr list -R github.com/ivanzaida/kontur --state open --limit 50 --json \
                           number,title,url,state,isDraft,headRefName,baseRefName,headRefOid,author,additions,\
                           deletions,changedFiles,updatedAt,reviewDecision";
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
  let command = gh_pr_list_command(&gh(), &repo("ivanzaida", "kontur"), SYNC_LIMIT);
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

/// GitHub's answer counting checks of the synthetic list's heads: one failed check run and two
/// successful commit statuses on the first, nothing known for the second, no checks on the third
/// (shaped like the captured answer).
const SYNTHETIC_CHECKS: &str = r#"{"data":{"repository":{"c0":{"oid":"HEAD0","statusCheckRollup":{"contexts":{"checkRunCountsByState":[{"state":"FAILURE","count":1},{"state":"SUCCESS","count":0}],"statusContextCountsByState":[{"state":"SUCCESS","count":2}]}}},"c1":null,"c2":{"oid":"HEAD2","statusCheckRollup":null}}}}"#;

fn heads(listed: &[PullRequest]) -> Vec<Revision> {
  listed
    .iter()
    .map(|pull_request| pull_request.head_sha().clone())
    .collect()
}

/// The check counts request for the synthetic list's heads.
fn checks_kontur() -> &'static str {
  static ARGS: OnceLock<String> = OnceLock::new();
  ARGS.get_or_init(|| {
    gh_check_rollups_command(&gh(), &repo("ivanzaida", "kontur"), &heads(&synthetic()))
      .args()
      .join(" ")
  })
}

fn synthetic_checks() -> String {
  let listed = synthetic();
  SYNTHETIC_CHECKS
    .replace("HEAD0", listed[0].head_sha().as_str())
    .replace("HEAD2", listed[2].head_sha().as_str())
}

#[test]
fn sync_reads_the_list_as_the_expected_account() {
  let mut answers = signed_in();
  answers.push((LIST_KONTUR, Ok(ok(fixture!("gh_pr_list_synthetic_open.stdout")))));
  answers.push((checks_kontur(), Ok(ok(&synthetic_checks()))));
  let (runner, result) = sync_with(answers, "ivanzaida");
  let listed = result.outcome.expect("the list");
  assert_eq!(listed.len(), 3);
  assert_eq!(result.check.result.signed_in_account(), Some(&login("ivanzaida")));
  // The checks are the counted ones, on each head.
  let first = synthetic()[0].number();
  let counted = listed
    .iter()
    .find(|pull_request| pull_request.number() == first)
    .expect("the first");
  let checks = counted.checks();
  assert_eq!((checks.passed, checks.failed, checks.total()), (2, 1, 3));
  let others = listed.iter().filter(|pull_request| pull_request.number() != first);
  assert!(others.map(PullRequest::checks).all(|checks| checks.total() == 0));
  assert_eq!(
    runner.ran().len(),
    4,
    "gh --version, auth status, the list and the checks"
  );
}

#[test]
fn check_rollups_ask_for_counts_of_exact_head_commits() {
  let heads = heads(&synthetic());
  let command = gh_check_rollups_command(&gh(), &repo("ivanzaida", "kontur"), &heads);
  // The query is the request body on standard input, never an argument.
  assert_eq!(
    command.args(),
    ["api", "graphql", "--hostname", "github.com", "--input", "-"]
  );
  let body: serde_json::Value = serde_json::from_str(command.stdin().expect("a request body")).expect("JSON");
  assert_eq!(
    body["variables"],
    serde_json::json!({"owner": "ivanzaida", "name": "kontur"})
  );
  let query = body["query"].as_str().expect("the query");
  for (index, head) in heads.iter().enumerate() {
    assert!(query.contains(&format!("c{index}:object(oid:\"{head}\")")));
  }
  assert!(query.contains("checkRunCountsByState") && query.contains("statusContextCountsByState"));
}

#[test]
fn check_rollups_of_a_large_repository_are_counted_by_state() {
  let vscode = repo("microsoft", "vscode");
  let listed = parse_gh_pr_list(&vscode, &ok(fixture!("gh_pr_list_vscode_open_50.stdout"))).expect("a readable list");
  assert_eq!(listed.len(), 50);
  let heads = heads(&listed);
  let answer = ok(fixture!("gh_api_graphql_checks_vscode_50.stdout"));
  let rollups = parse_gh_check_rollups(&vscode, &heads, &answer).expect("readable counts");
  assert_eq!(rollups.len(), 50);
  assert!(rollups.iter().zip(&heads).all(|(rollup, head)| &rollup.on == head));
  // #338037: two successful check runs and one in progress.
  assert_eq!(listed[0].number(), 338_037);
  assert_eq!((rollups[0].passed, rollups[0].pending, rollups[0].total()), (2, 1, 3));
  // #338031: 26 passed, 6 failed, 2 running.
  assert_eq!((rollups[3].passed, rollups[3].failed, rollups[3].pending), (26, 6, 2));
  let first = listed.into_iter().next().expect("a pull request");
  let with_checks = first.with_checks(rollups[0].clone()).expect("checks of its head");
  assert_eq!(with_checks.checks().total(), 3);
  assert_eq!(
    with_checks.with_checks(rollups[1].clone()),
    Err(PullRequestError::ChecksNotOnHead)
  );
}

#[test]
fn check_rollups_reject_unknown_states_and_other_commits() {
  let kontur = repo("ivanzaida", "kontur");
  let heads = heads(&synthetic());
  let unknown = synthetic_checks().replace("\"FAILURE\"", "\"SOMETHING_NEW\"");
  assert!(matches!(
    parse_gh_check_rollups(&kontur, &heads, &ok(&unknown)),
    Err(RepositoryError::Unrecognized { .. })
  ));
  let moved = synthetic_checks().replace(heads[0].as_str(), &"0".repeat(40));
  assert!(matches!(
    parse_gh_check_rollups(&kontur, &heads, &ok(&moved)),
    Err(RepositoryError::Unrecognized { .. })
  ));
}

#[test]
fn github_server_errors_are_not_unrecognized_output() {
  let vscode = repo("microsoft", "vscode");
  let server = |stderr: &str| parse_gh_pr_list(&vscode, &failed(stderr));
  assert!(matches!(
    server(fixture!("gh_pr_list_vscode_gateway_timeout.stderr")),
    Err(RepositoryError::ServerError { status: Some(504), .. })
  ));
  assert!(matches!(
    server(fixture!("gh_pr_list_bat_cut_off.stderr")),
    Err(RepositoryError::ServerError { status: None, .. })
  ));
  // Synthetic: the shape gh prints for a 502.
  assert!(matches!(
    server("HTTP 502: Bad Gateway (https://api.github.com/graphql)\n"),
    Err(RepositoryError::ServerError { status: Some(502), .. })
  ));
  // A client error stays what it was.
  assert!(matches!(
    server("HTTP 401: Bad credentials (https://api.github.com/graphql)\n"),
    Err(RepositoryError::NotSignedIn(_))
  ));
}

#[test]
fn sync_keeps_server_errors_typed() {
  let mut answers = signed_in();
  answers.push((
    LIST_KONTUR,
    Ok(failed(fixture!("gh_pr_list_vscode_gateway_timeout.stderr"))),
  ));
  let (_, result) = sync_with(answers, "ivanzaida");
  assert!(matches!(
    result.outcome,
    Err(SyncFailure::Repository(RepositoryError::ServerError {
      status: Some(504),
      ..
    }))
  ));
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
