//! A merge that fails for a reason other than the code ends in a state the person can retry from (ORC-365), against a
//! real repository: causes that pass are tried again by Orchester and recorded as its retries, *Try now* and *Stop
//! retrying* end the wait, a git that stops answering is recorded at its stage with its command and wait, files a
//! bring-in left changed are put back before any merge goes on, and a merge the app ended is found on the next start.
//! `cargo test -p orchester-workspace --test local_integration merge_`.

use std::{
  fs,
  num::NonZeroU32,
  panic::{AssertUnwindSafe, catch_unwind},
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  integration::{
    MergeError, MergeStep, PreflightCause, PreflightStage,
    attempt::{AttemptOutcome, AttemptStage, MergeAsker, MergeVia, OpenAttempt},
    merge_run,
  },
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_exec::{ProcessRunner, local_git::IntegrationLease, programs::Programs};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  integration::{IntegrationProblem, merge_in},
  merge_attempts::read,
  merge_left::{left_changed, put_files_back, recover_interrupted},
  merge_retry::{merge_retrying, retried, stop_retrying, try_now, waiting},
};

use super::support::{Fixture, git};

const PERSON: (MergeAsker, MergeVia) = (MergeAsker::Person, MergeVia::TaskPage);
const SILENCE: Duration = Duration::from_secs(120);

type Merging = thread::JoinHandle<Result<orchester_workspace::tasks::integration::Merged, IntegrationProblem>>;

/// The person's merge of the fixture's run, on a thread of its own as the task page merges.
fn merging(fixture: &Fixture) -> Merging {
  let root = fixture.root().to_owned();
  let run = fixture.run;
  thread::spawn(move || merge_retrying(&root, &Programs::processes(), (run, None, None), PERSON))
}

/// Waits until Orchester waits to try the fixture's merge again, or the merge ended; whether it waits.
fn until_waiting(fixture: &Fixture, merge: &Merging) -> bool {
  let deadline = Instant::now() + Duration::from_secs(60);
  while Instant::now() < deadline {
    if waiting(fixture.run).is_some() {
      return true;
    }
    if merge.is_finished() {
      return false;
    }
    thread::sleep(Duration::from_millis(20));
  }
  panic!("the merge neither waited nor ended");
}

fn askers(fixture: &Fixture) -> Vec<MergeAsker> {
  read::attempts_of(fixture.root(), fixture.run)
    .expect("the attempts read")
    .iter()
    .map(|recorded| recorded.attempt.asked_by())
    .collect()
}

fn retry(number: u32) -> MergeAsker {
  MergeAsker::Retry {
    number: NonZeroU32::new(number).expect("a try counts from 1"),
  }
}

fn held_lease() -> IntegrationProblem {
  IntegrationProblem::Preflight {
    stage: PreflightStage::IntegrationLock,
    cause: PreflightCause::Held,
  }
}

#[test]
fn merge_retry_a_held_lease_is_tried_again_by_orchester_and_lands_once_free() {
  let fixture = Fixture::new(false);
  let held = IntegrationLease::acquire(fixture.root()).expect("another operation's lease");
  let merge = merging(&fixture);
  assert!(until_waiting(&fixture, &merge));
  let wait = waiting(fixture.run).expect("waits");
  assert_eq!(wait.number.get(), 1);
  let not_merged = read::not_merged(fixture.root()).expect("read");
  assert_eq!(
    not_merged[0].retrying.and_then(|retrying| retrying.next),
    Some(wait.at),
    "Review › Merges tells when Orchester tries next"
  );
  drop(held);
  assert!(try_now(fixture.run), "Try now ends the wait");
  merge
    .join()
    .expect("the merge thread")
    .expect("merged by Orchester's retry");
  assert_eq!(askers(&fixture), [retry(1), MergeAsker::Person]);
  assert_eq!(waiting(fixture.run), None);
}

#[test]
fn merge_retry_gives_up_after_three_tries_each_recorded_as_orchesters() {
  let fixture = Fixture::new(false);
  let _held = IntegrationLease::acquire(fixture.root()).expect("another operation's lease");
  let merge = merging(&fixture);
  while until_waiting(&fixture, &merge) {
    try_now(fixture.run);
    thread::sleep(Duration::from_millis(20));
  }
  let ended = merge.join().expect("the merge thread");
  assert_eq!(ended.expect_err("still held"), held_lease());
  assert_eq!(askers(&fixture), [retry(3), retry(2), retry(1), MergeAsker::Person]);
  let not_merged = read::not_merged(fixture.root()).expect("read");
  assert_eq!(not_merged[0].retrying, None, "a person merges it again");
}

#[test]
fn merge_retry_stop_retrying_ends_it_as_not_merged() {
  let fixture = Fixture::new(false);
  let _held = IntegrationLease::acquire(fixture.root()).expect("another operation's lease");
  let merge = merging(&fixture);
  assert!(until_waiting(&fixture, &merge));
  assert!(stop_retrying(fixture.run));
  let ended = merge.join().expect("the merge thread");
  assert_eq!(ended.expect_err("not merged"), held_lease());
  assert_eq!(askers(&fixture), [MergeAsker::Person]);
  assert!(!try_now(fixture.run), "nothing waits any more");
}

#[test]
fn merge_retry_a_lasting_cause_is_not_tried_again() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("task.txt"), "main's task\n").expect("main's edit");
  git(fixture.root(), &["commit", "-am", "main changes the task file"]);
  let merge = merging(&fixture);
  assert!(!until_waiting(&fixture, &merge));
  let ended = merge.join().expect("the merge thread");
  assert!(
    matches!(ended, Err(IntegrationProblem::Merge(MergeError::Conflict { .. }))),
    "{ended:?}"
  );
  assert_eq!(askers(&fixture), [MergeAsker::Person]);
}

/// Git as real processes run it, except for the commands `fake` answers itself.
struct Faked<F>(ProcessRunner, F);

impl<F: Fn(&[String]) -> Option<Result<CommandOutput, CommandError>> + Send + Sync> CommandRunner for Faked<F> {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    (self.1)(command.args()).unwrap_or_else(|| self.0.run(command))
  }
}

fn faked(fake: impl Fn(&[String]) -> Option<Result<CommandOutput, CommandError>> + Send + Sync + 'static) -> Programs {
  Programs::with_runner(Arc::new(Faked(ProcessRunner::new(Duration::from_secs(60)), fake)))
}

fn has(args: &[String], word: &str) -> bool {
  args.iter().any(|arg| arg == word)
}

fn failed(stderr: &str) -> Result<CommandOutput, CommandError> {
  Ok(CommandOutput {
    code: Some(1),
    stdout: String::new(),
    stderr: stderr.to_owned(),
  })
}

/// While Orchester's try goes on, the run still shows it, due since it began, and *Stop retrying* asked then ends the
/// merge once that try did not land, though its cause passes.
#[test]
fn merge_retry_a_try_under_way_shows_due_and_stop_retrying_ends_it_after() {
  let fixture = Fixture::new(false);
  let released = Arc::new(AtomicBool::new(false));
  let gate = released.clone();
  let programs = faked(move |args| {
    has(args, "--ff-only").then(|| {
      while !gate.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(10));
      }
      Err(CommandError::Failed(
        "The process cannot access the file because it is being used by another process.".to_owned(),
      ))
    })
  });
  let (root, run) = (fixture.root().to_owned(), fixture.run);
  let merge = thread::spawn(move || retried(&root, &programs, (run, None, None), PERSON, Err(held_lease())));
  assert!(until_waiting(&fixture, &merge));
  assert!(try_now(run), "Try now ends the wait");
  let deadline = Instant::now() + Duration::from_secs(60);
  while waiting(run).is_none_or(|wait| wait.at > Timestamp::now()) {
    assert!(Instant::now() < deadline, "the try never showed as under way");
    thread::sleep(Duration::from_millis(10));
  }
  assert_eq!(waiting(run).map(|wait| wait.number.get()), Some(1));
  assert!(stop_retrying(run), "Stop retrying reaches the try under way");
  released.store(true, Ordering::SeqCst);
  let ended = merge.join().expect("the merge thread");
  assert!(ended.as_ref().expect_err("not merged").passes(), "{ended:?}");
  assert_eq!(askers(&fixture), [retry(1)], "no try after the one under way");
  assert_eq!(waiting(run), None);
}

#[test]
fn merge_watchdog_git_that_stops_answering_is_recorded_with_its_command_and_wait_and_not_retried() {
  let fixture = Fixture::new(false);
  let programs = faked(|args| has(args, "--ff-only").then_some(Err(CommandError::Silent { after: SILENCE })));
  let ended = merge_retrying(fixture.root(), &programs, (fixture.run, None, None), PERSON);
  let Err(IntegrationProblem::Merge(MergeError::Unanswered { step, .. })) = &ended else {
    panic!("git stopped answering: {ended:?}");
  };
  assert_eq!(*step, MergeStep::BringIn);
  let message = ended.as_ref().expect_err("not merged").message(", ");
  assert_eq!(message.key, "runs.merge.failure.unanswered.bringIn");
  let recorded = read::attempts_of(fixture.root(), fixture.run).expect("read");
  assert_eq!(recorded.len(), 1, "a disk that stops answering is not tried again");
  let attempt = &recorded[0].attempt;
  assert_eq!(
    attempt.outcome(),
    &AttemptOutcome::GitStopped {
      stage: AttemptStage::Merge
    }
  );
  let cause = attempt.cause().expect("a cause");
  assert!(
    cause.contains("bringing the merge in") && cause.contains("--ff-only") && cause.contains("120 s"),
    "{cause}"
  );
  assert_eq!(
    fs::read_to_string(fixture.root().join("task.txt")).expect("there"),
    "task base\n"
  );
}

/// Programs whose fast-forward writes the run's `task.txt` into the checkout at `root` and stops, and whose put-back
/// cannot write it.
fn stops_part_way(root: PathBuf) -> Programs {
  faked(move |args| {
    if has(args, "--ff-only") {
      fs::write(root.join("task.txt"), "delivered task\n").expect("git wrote the file");
      return Some(failed("fatal: stopped part way"));
    }
    has(args, "checkout-index").then(|| failed("error: task.txt: Input/output error"))
  })
}

#[test]
fn merge_put_back_restores_the_files_and_try_again_is_refused_until_then() {
  let fixture = Fixture::new(false);
  let processes = Programs::processes();
  let refused = merge_in(
    fixture.root(),
    &stops_part_way(fixture.root().to_owned()),
    (fixture.run, None),
  )
  .expect_err("left changed");
  assert!(
    matches!(refused, IntegrationProblem::Merge(MergeError::LeftChanged { .. })),
    "{refused:?}"
  );
  assert_eq!(
    left_changed(fixture.root(), &processes, fixture.run).expect("read"),
    ["task.txt"],
    "the run is Stopped while bringing the merge in, with its files"
  );
  let again = merge_in(fixture.root(), &processes, (fixture.run, None)).expect_err("refused");
  assert_eq!(again.message(", ").key, "runs.merge.failure.notPutBack");

  put_files_back(fixture.root(), &processes, fixture.run).expect("put back");
  assert_eq!(
    fs::read_to_string(fixture.root().join("task.txt")).expect("there"),
    "task base\n"
  );
  assert!(
    left_changed(fixture.root(), &processes, fixture.run)
      .expect("read")
      .is_empty()
  );
  merge_in(fixture.root(), &processes, (fixture.run, None)).expect("merged once put back");
}

#[test]
fn merge_interrupted_is_found_on_the_next_start_recorded_and_its_leftovers_cleared() {
  let fixture = Fixture::new(false);
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  let stored = db.run(fixture.run).expect("read").expect("the run");
  let task = db.task(stored.task()).expect("read").expect("the task");
  // The app ends while git brings the merge in, after it wrote the run's file.
  let root = fixture.root().to_owned();
  let ends = Faked(ProcessRunner::new(Duration::from_secs(60)), move |args: &[String]| {
    if has(args, "--ff-only") {
      fs::write(root.join("task.txt"), "delivered task\n").expect("git wrote the file");
      panic!("the app ended during the merge");
    }
    None
  });
  let ended = catch_unwind(AssertUnwindSafe(|| {
    merge_run(
      &ends,
      &PathBuf::from("git"),
      fixture.root(),
      (&stored, task.key()),
      Timestamp::now(),
    )
  }));
  assert!(ended.is_err());
  db.open_merge_attempt(&OpenAttempt {
    run: fixture.run,
    at: Timestamp::from_unix_millis(10),
    asked_by: MergeAsker::Person,
    via: MergeVia::TaskPage,
    stage: AttemptStage::Merge,
  })
  .expect("the attempt the app left open");
  let common = git(
    fixture.root(),
    &["rev-parse", "--path-format=absolute", "--git-common-dir"],
  );
  let temporary = format!("{common}/orchester-merge-{}", fixture.run);
  git(fixture.root(), &["worktree", "add", "--detach", &temporary]);

  let programs = Programs::processes();
  assert_eq!(
    recover_interrupted(fixture.root(), &programs).expect("recovered"),
    [fixture.run]
  );
  assert!(fs::metadata(&temporary).is_err(), "the temporary worktree went");
  assert!(db.open_merge_attempts().expect("read").is_empty());
  let attempt = &read::attempts_of(fixture.root(), fixture.run).expect("read")[0].attempt;
  assert_eq!(
    attempt.outcome(),
    &AttemptOutcome::Interrupted {
      stage: AttemptStage::Merge
    }
  );
  assert!(
    attempt.cause().is_some_and(|cause| cause.contains("task.txt")),
    "{attempt:?}"
  );
  assert_eq!(
    left_changed(fixture.root(), &programs, fixture.run).expect("read"),
    ["task.txt"]
  );
  put_files_back(fixture.root(), &programs, fixture.run).expect("put back");
  merge_in(fixture.root(), &programs, (fixture.run, None)).expect("merged after it");
}
