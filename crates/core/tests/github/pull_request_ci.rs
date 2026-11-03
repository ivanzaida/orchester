//! A pull request's CI: `gh pr checks --json` and `gh run view --job --log-failed`, against synthetic answers in
//! the shape gh 2.97 writes (a pull request's checks cannot be made to fail, run and wait on demand): each check's
//! state, a failed GitHub Actions job's address and log tail, no checks, a missing pull request, and a read that
//! stops at the account (not signed in, no `gh`) before it asks for checks.

use orchester_core::{
  command::{CommandError, CommandOutput},
  connection::{GitHubHost, GitHubLogin, GitHubState},
  github::GhCli,
  id::ConnectionId,
  pull_request::{
    SyncFailure,
    ci::{
      CiState, CiSummary, LOG_TAIL_LINES, gh_job_log_command, gh_pr_checks_command, parse_gh_job_log,
      parse_gh_pr_checks, read_ci,
    },
  },
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryError, RepositoryLink, RepositoryView},
  run::BranchName,
};

use crate::{
  connection::github_connection,
  replay::{Replay, failed, fixture, gh, now, ok},
};

const VERSION: &str = "--version";
const AUTH_STATUS: &str = "auth status --hostname github.com --json hosts";
const CHECKS_463: &str = "pr checks 463 -R github.com/dtolnay/anyhow --json name,state,bucket,link,workflow";
const LOG_CLIPPY: &str = "run view --job 52011223345 -R github.com/dtolnay/anyhow --log-failed";
const MIXED: &str = fixture!("gh_pr_checks_synthetic_mixed.stdout");
const LOG: &str = fixture!("gh_run_view_log_failed_synthetic.stdout");

fn anyhow() -> GitHubRepo {
  GitHubRepo::new(GitHubHost::github_com(), "dtolnay", "anyhow").expect("valid repository")
}

fn link() -> RepositoryLink {
  let view = RepositoryView {
    repo: anyhow(),
    default_branch: BranchName::new("master").expect("valid branch"),
    permission: RepoPermission::Read,
  };
  let expected = GitHubLogin::new("ivanzaida").expect("valid login");
  RepositoryLink::new(ConnectionId::generate(), RemoteName::origin(), view, expected, now())
}

fn exited(code: i32, stdout: &str) -> CommandOutput {
  CommandOutput {
    code: Some(code),
    stdout: stdout.to_owned(),
    stderr: String::new(),
  }
}

#[test]
fn ci_commands_ask_for_the_checks_and_a_jobs_failed_steps() {
  assert_eq!(gh_pr_checks_command(&gh(), &anyhow(), 463).args().join(" "), CHECKS_463);
  assert_eq!(
    gh_job_log_command(&gh(), &anyhow(), 52_011_223_345).args().join(" "),
    LOG_CLIPPY
  );
}

#[test]
fn ci_checks_give_each_state_and_the_failed_ones_address() {
  let checks = parse_gh_pr_checks(&anyhow(), 463, &ok(MIXED)).expect("readable checks");
  let states: Vec<(&str, CiState)> = checks.iter().map(|check| (check.name.as_str(), check.state)).collect();
  assert_eq!(
    states,
    [
      ("test (stable)", CiState::Passed),
      ("clippy", CiState::Failed),
      ("test (nightly)", CiState::Running),
      ("miri", CiState::Queued),
      ("docs", CiState::Cancelled),
      ("publish", CiState::Skipped),
      ("ci/external", CiState::Failed),
    ]
  );
  let clippy = &checks[1];
  assert_eq!(
    clippy.url.as_deref(),
    Some("https://github.com/dtolnay/anyhow/actions/runs/18234567890/job/52011223345")
  );
  assert_eq!(clippy.job, Some(52_011_223_345));
  assert_eq!(clippy.workflow.as_deref(), Some("CI"));
  let external = &checks[6];
  assert_eq!(external.url.as_deref(), Some("https://ci.example.com/builds/4410"));
  assert_eq!((external.job, external.workflow.as_deref()), (None, None));
}

#[test]
fn ci_checks_written_before_a_pending_or_failing_exit_are_read() {
  for code in [1, 8] {
    let checks = parse_gh_pr_checks(&anyhow(), 463, &exited(code, MIXED)).expect("readable checks");
    assert_eq!(checks.len(), 7, "exit code {code}");
  }
}

#[test]
fn ci_head_without_checks_has_none_and_a_missing_pull_request_is_not_found() {
  let none = parse_gh_pr_checks(
    &anyhow(),
    463,
    &failed(fixture!("gh_pr_checks_synthetic_no_checks.stderr")),
  );
  assert_eq!(none, Ok(Vec::new()));
  let missing = parse_gh_pr_checks(&anyhow(), 99_999, &failed(fixture!("gh_pr_view_not_found.stderr")));
  assert_eq!(
    missing,
    Err(RepositoryError::PullRequestNotFound {
      repo: anyhow(),
      number: 99_999
    })
  );
  let signed_out = parse_gh_pr_checks(&anyhow(), 463, &failed(fixture!("gh_repo_view_bad_credentials.stderr")));
  assert!(
    matches!(signed_out, Err(RepositoryError::NotSignedIn(_))),
    "{signed_out:?}"
  );
}

#[test]
fn ci_log_tail_keeps_the_last_lines_without_job_step_and_time() {
  let tail = parse_gh_job_log(&anyhow(), &ok(LOG))
    .expect("a readable log")
    .expect("a log");
  let lines: Vec<&str> = tail.lines().collect();
  assert_eq!(lines.len(), LOG_TAIL_LINES);
  assert_eq!(lines.first(), Some(&"##[endgroup]"));
  assert_eq!(
    &lines[lines.len() - 3..],
    [
      "  --> src/error.rs:812:9",
      "error: could not compile `anyhow` (lib) due to 1 previous error",
      "##[error]Process completed with exit code 101.",
    ]
  );
  assert!(!tail.contains("Run cargo clippy\t"), "{tail}");
  assert_eq!(parse_gh_job_log(&anyhow(), &ok("\n")), Ok(None));
}

#[test]
fn ci_read_gives_the_failed_jobs_log_and_how_the_checks_stand() {
  let runner = Replay::new([
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(ok(fixture!("gh_auth_status_signed_in.stdout")))),
    (CHECKS_463, Ok(ok(MIXED))),
    (LOG_CLIPPY, Ok(ok(LOG))),
  ]);
  let connection = github_connection();
  let read = read_ci(&GhCli::new(&runner, &connection), &link(), 463, now());
  let checks = read.outcome.expect("the checks were read");
  assert_eq!(checks.summary(), CiSummary::Failed);
  assert!(checks.is_pending(), "two checks still run or wait");
  let clippy = &checks.checks[1];
  assert!(
    clippy
      .log_tail
      .as_deref()
      .is_some_and(|tail| tail.ends_with("exit code 101.")),
    "{clippy:?}"
  );
  // A commit status has no Actions job, so no log is asked for; cancelled and passed ones have none either.
  assert!(
    checks
      .checks
      .iter()
      .filter(|check| check.name != "clippy")
      .all(|check| check.log_tail.is_none())
  );
  assert_eq!(
    runner.ran().iter().filter(|command| command.args()[0] == "run").count(),
    1
  );
}

#[test]
fn ci_read_says_why_without_sign_in_or_gh_and_asks_for_no_checks() {
  let signed_out = CommandOutput {
    code: Some(1),
    stdout: fixture!("gh_auth_status_not_signed_in.stdout").to_owned(),
    stderr: fixture!("gh_auth_status_not_signed_in.stderr").to_owned(),
  };
  let runner = Replay::new([
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(signed_out)),
  ]);
  let connection = github_connection();
  let read = read_ci(&GhCli::new(&runner, &connection), &link(), 463, now());
  assert_eq!(read.outcome, Err(SyncFailure::Account(GitHubState::NotSignedIn)));
  let missing = Replay::new([(VERSION, Err(CommandError::NotFound))]);
  let read = read_ci(&GhCli::new(&missing, &connection), &link(), 463, now());
  assert_eq!(read.outcome, Err(SyncFailure::Account(GitHubState::ProgramMissing)));
}
