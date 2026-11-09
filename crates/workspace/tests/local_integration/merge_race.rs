//! A merge asked for the moment a run ends. The run's last Git program may still be journaled as running, which a
//! moment later it no longer is: the merge waits for that proof a short while and merges. A program that really runs
//! is never taken for stopped, and the merge refuses. A folder an earlier operation left holds nothing back.

use std::{
  fs,
  process::{Child, Command, Stdio},
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  id::RunId,
  integration::{
    MergeKind,
    local::{ExecutorIdentity, LocalError, LocalProcessStore},
  },
};
use orchester_exec::{local_git::LocalGitError, programs::Programs};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  integration::{IntegrationProblem, merge_in},
  local_integration::{CheckoutHolder, IntegrationError, LocalIntegration},
};

use super::support::Fixture;

/// A Git program that runs until its input ends, and the run's journal saying it started it and has not finished: the
/// state a merge finds when it is asked at the moment the run's last program is closing.
fn journaled_running(fixture: &Fixture) -> (Child, u64) {
  let mut review = fixture.open();
  review.refresh().expect("review");
  drop(review);
  let child = Command::new("git")
    .args(["hash-object", "--stdin"])
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .expect("a program that waits for input");
  let mut identity = orchester_exec::run_processes::identity::capture(child.id())
    .expect("read the program")
    .expect("it is running");
  identity.executor = Some(ExecutorIdentity {
    pid: identity.pid,
    started: identity.started,
  });
  // No job or group of this name exists, so only the program itself keeps the receipt from being proved stopped.
  identity.containment = Some(orchester_exec::local_git::digest(b"a job nothing runs in"));
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  let sequence = db.begin_local_process(fixture.run).expect("receipt");
  db.local_process_started(fixture.run, sequence, Some(&identity))
    .expect("started");
  (child, sequence)
}

fn finished(fixture: &Fixture) -> bool {
  ProjectDb::open(fixture.root())
    .expect("db")
    .local_process(fixture.run)
    .expect("receipt")
    .expect("exists")
    .finished
}

/// A merge asked while the run's own Git program is journaled as running waits for the program to end, as the
/// receipt of one that is closing does a moment later, and then merges.
#[test]
fn merge_race_a_program_closing_when_the_merge_is_asked_does_not_refuse_it() {
  let fixture = Fixture::new(false);
  let (mut child, sequence) = journaled_running(&fixture);
  let (root, run) = (fixture.root().to_owned(), fixture.run);
  let closing = thread::spawn(move || {
    thread::sleep(Duration::from_millis(700));
    child.kill().expect("the program ends");
    child.wait().expect("the program is reaped");
    ProjectDb::open(&root)
      .expect("db")
      .finish_local_process(run, sequence)
      .expect("the receipt ends with the program");
  });
  let merged = merge_in(&fixture.worktrees(), &Programs::processes(), (fixture.run, None)).expect("merges");
  closing.join().expect("the program closed");
  assert_eq!(merged.merge.kind, MergeKind::FastForward);
}

/// Opening the run's local integration while the program that ran it is still closing proves it stopped once it has.
#[test]
fn merge_race_a_program_that_stops_while_the_run_opens_is_proved_stopped() {
  let fixture = Fixture::new(false);
  let (mut child, _) = journaled_running(&fixture);
  let closing = thread::spawn(move || {
    thread::sleep(Duration::from_millis(700));
    child.kill().expect("the program ends");
    child.wait().expect("the program is reaped");
  });
  let opened = LocalIntegration::open(fixture.root(), fixture.run);
  closing.join().expect("the program closed");
  assert!(opened.is_ok(), "{:?}", opened.err());
  assert!(finished(&fixture), "the receipt of the program that closed is ended");
}

/// A program that keeps running is never taken for stopped: the merge refuses after the wait and tells the program
/// is running, and the program stays journaled.
#[test]
fn merge_race_a_program_that_keeps_running_refuses_the_merge_and_stays_journaled() {
  let fixture = Fixture::new(false);
  let (mut child, _) = journaled_running(&fixture);
  let asked = Instant::now();
  let merge = merge_in(&fixture.worktrees(), &Programs::processes(), (fixture.run, None));
  let waited = asked.elapsed();
  let refused = merge.expect_err("a program is running");
  assert_eq!(refused, IntegrationProblem::HeldLocally(CheckoutHolder::RunningProgram));
  assert_eq!(refused.message("").key, "runs.merge.failure.programRunning");
  assert!(refused.passes(), "Orchester tries the merge again by itself");
  assert!(
    waited >= Duration::from_secs(2),
    "the merge looked again first: {waited:?}"
  );
  let reopened = LocalIntegration::open(fixture.root(), fixture.run);
  child.kill().expect("stop the program");
  child.wait().expect("reap the program");
  assert!(
    matches!(
      reopened,
      Err(
        IntegrationError::State(LocalError::ProcessUncertain)
          | IntegrationError::Git(LocalGitError::State(LocalError::ProcessUncertain))
      )
    ),
    "{:?}",
    reopened.err()
  );
  assert!(!finished(&fixture), "a program that ran stays journaled");
}

#[test]
fn merge_race_a_folder_an_earlier_operation_left_does_not_hold_the_merge_back() {
  let fixture = Fixture::new(false);
  let leftover = fixture
    .root()
    .join(".orchester/local-integration")
    .join(RunId::generate().to_string());
  fs::create_dir_all(&leftover).expect("folder");
  fs::write(leftover.join(".orchester-local-owner"), b"").expect("owner marker");
  let merged = merge_in(&fixture.worktrees(), &Programs::processes(), (fixture.run, None)).expect("merges");
  assert_eq!(merged.merge.kind, MergeKind::FastForward);
  assert!(
    leftover.join(".orchester-local-owner").exists(),
    "it is not this merge's to remove"
  );
}
