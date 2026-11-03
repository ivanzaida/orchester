//! A run's pull request's CI through a fake `gh` answering as gh 2.97 writes `gh pr checks --json` and
//! `gh run view --log-failed`: the run's pull request is the one of its branch the last sync stored; its checks each
//! with a state, the failed Actions job with its log tail; a read answers for 30 s and a screen reads again while a
//! check runs; no `gh`, no sign-in, no link and no pull request are states of their own, never a pass.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  pull_request::{
    PullRequestState,
    ci::{CiState, CiSummary},
  },
  run::Run,
  store::PullRequestStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::pull_requests::ci::{CiReads, PullRequestCi, read_ci_in, run_pull_request};
use parking_lot::Mutex;

use super::{
  link, pull_request, repo,
  support::{Fixture, at},
};

/// What the fake's `gh pr checks` writes: a passed and a failed Actions job, one still running, and a commit
/// status.
const CHECKS: &str = r#"[{"bucket":"pass","link":"https://github.com/ivanzaida/kontur/actions/runs/900/job/9001","name":"test","state":"SUCCESS","workflow":"CI"},{"bucket":"fail","link":"https://github.com/ivanzaida/kontur/actions/runs/900/job/9002","name":"clippy","state":"FAILURE","workflow":"CI"},{"bucket":"pending","link":"https://github.com/ivanzaida/kontur/actions/runs/900/job/9003","name":"e2e","state":"IN_PROGRESS","workflow":"CI"},{"bucket":"pass","link":"https://ci.example.com/b/1","name":"ci/external","state":"SUCCESS","workflow":""}]"#;
const LOG: &str = "clippy\tRun cargo clippy\t2026-11-03T10:15:04.0000000Z error: this `if` has identical blocks\n\
                   clippy\tRun cargo clippy\t2026-11-03T10:15:04.3000000Z ##[error]Process completed with exit code 101.\n";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
  Ok,
  SignedOut,
  Missing,
}

/// `gh` answering the reads of a pull request's checks.
struct FakeGh {
  mode: Mutex<Mode>,
  /// The `pr checks` requests it answered.
  checks: AtomicUsize,
}

impl FakeGh {
  fn new(mode: Mode) -> Arc<Self> {
    Arc::new(Self {
      mode: Mutex::new(mode),
      checks: AtomicUsize::new(0),
    })
  }

  fn checks(&self) -> usize {
    self.checks.load(Ordering::SeqCst)
  }
}

fn answer(code: i32, stdout: &str, stderr: &str) -> Result<CommandOutput, CommandError> {
  Ok(CommandOutput {
    code: Some(code),
    stdout: stdout.to_owned(),
    stderr: stderr.to_owned(),
  })
}

impl CommandRunner for FakeGh {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let args = command.args().join(" ");
    let mode = *self.mode.lock();
    if mode == Mode::Missing {
      return Err(CommandError::NotFound);
    }
    if args == "--version" {
      return answer(0, "gh version 2.97.0 (2026-07-31)\n", "");
    }
    if args.starts_with("auth status") {
      return match mode {
        Mode::SignedOut => answer(1, "{\"hosts\":{}}", "You are not logged into any GitHub hosts."),
        Mode::Ok | Mode::Missing => answer(
          0,
          r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivanzaida","scopes":"repo"}]}}"#,
          "",
        ),
      };
    }
    if args.starts_with("pr checks 142 ") {
      self.checks.fetch_add(1, Ordering::SeqCst);
      // gh exits with 8 while a check is pending, after writing them.
      return answer(8, CHECKS, "");
    }
    if args.starts_with("pr checks 77 ") {
      return answer(
        1,
        "",
        "GraphQL: Could not resolve to a PullRequest with the number of 77. (repository.pullRequest)",
      );
    }
    if args.starts_with("run view --job 9002 ") {
      return answer(0, LOG, "");
    }
    panic!("`{args}` is not a read of checks");
  }
}

/// A linked project whose run's branch has open pull request 142, and a run without one; `gh` is `gh`.
fn linked(gh: &Arc<FakeGh>) -> (Fixture, Run, Run, Programs) {
  let mut fixture = Fixture::keyed("KON");
  let task = fixture.task("Add retry to webhook sender", at(10));
  let with = fixture.started_run(task, at(20));
  let other = fixture.task("Typed errors per module", at(11));
  let without = fixture.started_run(other, at(21));
  link(&mut fixture);
  let stored = [
    pull_request(142, with.branch().as_str(), PullRequestState::Open, None, 900),
    pull_request(131, with.branch().as_str(), PullRequestState::Merged, None, 950),
  ];
  fixture
    .db
    .replace_pull_requests(&repo(), &stored, at(1_000))
    .expect("the pull requests are stored");
  (fixture, with, without, Programs::with_runner(gh.clone()))
}

fn read(ci: PullRequestCi) -> orchester_workspace::pull_requests::ci::ReadCi {
  match ci {
    PullRequestCi::Read(read) => read,
    other => panic!("the checks were read: {other:?}"),
  }
}

#[test]
fn ci_of_a_runs_pull_request_gives_each_check_and_the_failed_ones_log() {
  let gh = FakeGh::new(Mode::Ok);
  let (fixture, with, without, programs) = linked(&gh);
  assert_eq!(
    run_pull_request(&fixture.db, &with).expect("stored"),
    Some(142),
    "its open one, not the merged one"
  );
  assert_eq!(run_pull_request(&fixture.db, &without).expect("stored"), None);
  let now = Timestamp::now();
  let ci = read_ci_in(fixture.repo.path(), &programs, 142, now).expect("a read");
  assert_eq!(
    ci.next_read_at().map(|at| at.unix_millis() - now.unix_millis()),
    Some(30_000)
  );
  let read = read(ci);
  assert_eq!(read.checks.summary(), CiSummary::Failed);
  let states: Vec<CiState> = read.checks.checks.iter().map(|check| check.state).collect();
  assert_eq!(
    states,
    [CiState::Passed, CiState::Failed, CiState::Running, CiState::Passed]
  );
  let clippy = &read.checks.checks[1];
  assert_eq!(clippy.name, "clippy");
  assert_eq!(
    clippy.url.as_deref(),
    Some("https://github.com/ivanzaida/kontur/actions/runs/900/job/9002")
  );
  assert_eq!(
    clippy.log_tail.as_deref(),
    Some("error: this `if` has identical blocks\n##[error]Process completed with exit code 101.")
  );
}

#[test]
fn ci_reads_answer_for_30_seconds() {
  let gh = FakeGh::new(Mode::Ok);
  let (fixture, _, _, programs) = linked(&gh);
  let reads = CiReads::default();
  let project = (fixture.db.project().id(), fixture.repo.path());
  let now = Timestamp::now();
  let later = |seconds: i64| Timestamp::from_unix_millis(now.unix_millis() + seconds * 1000);
  reads.read(project, &programs, 142, now).expect("a read");
  reads.read(project, &programs, 142, later(10)).expect("a read");
  assert_eq!(gh.checks(), 1, "a read 10 s ago answers");
  reads.read(project, &programs, 142, later(31)).expect("a read");
  assert_eq!(gh.checks(), 2, "31 s later GitHub is asked again");
}

#[test]
fn ci_without_gh_sign_in_link_or_pull_request_says_which() {
  let gh = FakeGh::new(Mode::Missing);
  let (fixture, _, _, programs) = linked(&gh);
  let root = fixture.repo.path();
  let now = Timestamp::now();
  assert_eq!(
    read_ci_in(root, &programs, 142, now).expect("a read"),
    PullRequestCi::GhMissing
  );
  *gh.mode.lock() = Mode::SignedOut;
  assert_eq!(
    read_ci_in(root, &programs, 142, now).expect("a read"),
    PullRequestCi::NotSignedIn
  );
  *gh.mode.lock() = Mode::Ok;
  assert_eq!(
    read_ci_in(root, &programs, 77, now).expect("a read"),
    PullRequestCi::NoPullRequest
  );
  assert_eq!(gh.checks(), 0, "no read of 142 got past the account");
  for unread in [
    PullRequestCi::GhMissing,
    PullRequestCi::NotSignedIn,
    PullRequestCi::NoPullRequest,
  ] {
    assert_eq!(unread.next_read_at(), None);
  }
  let unlinked = Fixture::keyed("KON");
  assert_eq!(
    read_ci_in(unlinked.repo.path(), &programs, 142, now).expect("a read"),
    PullRequestCi::NotLinked
  );
}
