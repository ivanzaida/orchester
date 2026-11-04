//! Every merge attempt is recorded (ORC-361): who asked, from where, what came of it, why it did not land and how
//! each stage went, against a real repository; and what Review › Merges reads of them. `cargo test -p
//! orchester-workspace --test local_integration merge_attempts`.

use std::{fs, num::NonZeroU32, path::PathBuf, sync::Arc, time::Duration};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  integration::{
    MergeError, MergeKind,
    attempt::{AttemptOutcome, AttemptStage, CauseKind, MergeAsker, MergeVia, OpenAttempt, StageResult},
  },
  store::{MergeStore, RunStore},
  time::Timestamp,
};
use orchester_exec::{ProcessRunner, local_git::IntegrationLease, programs::Programs};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  data::ProjectTasks,
  integration::{IntegrationProblem, merge_in, merge_in_as},
  merge_attempts::read::{self, ReviewIn, SinceReview},
};

use super::support::{Fixture, git};

const PERSON: (MergeAsker, MergeVia) = (MergeAsker::Person, MergeVia::TaskPage);

fn attempts(fixture: &Fixture) -> Vec<orchester_core::integration::attempt::RecordedAttempt> {
  read::attempts_of(fixture.root(), fixture.run).expect("the attempts read")
}

fn result_of(stages: &[(AttemptStage, StageResult)], stage: AttemptStage) -> StageResult {
  stages
    .iter()
    .find(|(listed, _)| *listed == stage)
    .map(|(_, result)| *result)
    .expect("the stage is listed")
}

#[test]
fn merge_attempts_task_page_merge_records_who_where_and_every_stage_passed() {
  let fixture = Fixture::new(false);
  let merged = merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("merged");
  let recorded = attempts(&fixture);
  assert_eq!(recorded.len(), 1);
  let attempt = &recorded[0].attempt;
  assert_eq!((attempt.asked_by(), attempt.via()), PERSON);
  assert_eq!(
    attempt.outcome(),
    &AttemptOutcome::Merged {
      revision: merged.merge.revision.clone(),
      kind: MergeKind::FastForward,
    }
  );
  assert_eq!(attempt.cause(), None);
  let stages: Vec<AttemptStage> = attempt.stages().iter().map(|(stage, _)| *stage).collect();
  assert_eq!(
    stages,
    [
      AttemptStage::ProjectDatabase,
      AttemptStage::IntegrationLock,
      AttemptStage::ScratchFolder,
      AttemptStage::CheckoutIdentity,
      AttemptStage::GitMetadata,
      AttemptStage::ExistingResult,
      AttemptStage::Merge,
      AttemptStage::Record,
    ],
    "a person's attempt passes no policy, and lists the stages in the order they run"
  );
  assert!(
    attempt
      .stages()
      .iter()
      .all(|(_, result)| *result == StageResult::Passed)
  );
  assert!(read::not_merged(fixture.root()).expect("read").is_empty());
  let page = read::merged_by_day(fixture.root(), None, 10).expect("read");
  assert_eq!(page.days.len(), 1);
  assert_eq!(page.days[0].day, merged.merge.merged_at.local_day_start());
  assert_eq!(page.days[0].runs[0].1.run, fixture.run);
  assert_eq!(page.next, None);
}

#[test]
fn merge_attempts_refused_at_a_preflight_stage_keep_the_cause_until_a_merge_lands() {
  let fixture = Fixture::new(false);
  let held = IntegrationLease::acquire(fixture.root()).expect("another operation's lease");
  merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect_err("held");
  let refused = &attempts(&fixture)[0].attempt;
  assert_eq!(
    refused.outcome(),
    &AttemptOutcome::Refused {
      stage: AttemptStage::IntegrationLock
    }
  );
  assert_eq!(refused.cause(), Some("held by another operation"));
  assert_eq!(refused.kind(), Some(CauseKind::HeldByOperation));
  let stages = refused.stages();
  assert_eq!(result_of(stages, AttemptStage::ProjectDatabase), StageResult::Passed);
  assert_eq!(result_of(stages, AttemptStage::IntegrationLock), StageResult::Refused);
  assert_eq!(result_of(stages, AttemptStage::Merge), StageResult::NotReached);
  let not_merged = read::not_merged(fixture.root()).expect("read");
  assert_eq!(not_merged.len(), 1);
  assert_eq!(not_merged[0].attempt.attempt.run(), fixture.run);
  assert_eq!(not_merged[0].retrying, None, "a person merges it again");
  drop(held);
  merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("merged");
  assert_eq!(attempts(&fixture).len(), 2);
  assert!(
    read::not_merged(fixture.root()).expect("read").is_empty(),
    "merged since"
  );
}

#[test]
fn merge_attempts_conflicts_keep_the_files_both_sides_changed() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("task.txt"), "main's task\n").expect("main's edit");
  git(fixture.root(), &["commit", "-am", "main changes the task file"]);
  let refused = merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect_err("conflicts");
  assert!(
    matches!(refused, IntegrationProblem::Merge(MergeError::Conflict { .. })),
    "{refused:?}"
  );
  let attempt = &attempts(&fixture)[0].attempt;
  assert_eq!(
    attempt.outcome(),
    &AttemptOutcome::Conflicts {
      files: vec!["task.txt".to_owned()]
    }
  );
  assert_eq!(
    result_of(attempt.stages(), AttemptStage::ExistingResult),
    StageResult::Passed
  );
  assert_eq!(result_of(attempt.stages(), AttemptStage::Merge), StageResult::Refused);
  assert_eq!(
    result_of(attempt.stages(), AttemptStage::Record),
    StageResult::NotReached
  );
}

/// Git that does not answer: every program it is asked to run times out.
struct Silent;

impl CommandRunner for Silent {
  fn run(&self, _: &Command) -> Result<CommandOutput, CommandError> {
    Err(CommandError::TimedOut)
  }
}

#[test]
fn merge_attempts_git_that_stops_answering_is_told_at_its_stage() {
  let fixture = Fixture::new(false);
  let programs = Programs::with_runner(Arc::new(Silent));
  merge_in(fixture.root(), &programs, (fixture.run, None)).expect_err("git did not answer");
  let attempt = &attempts(&fixture)[0].attempt;
  assert_eq!(
    attempt.outcome(),
    &AttemptOutcome::GitStopped {
      stage: AttemptStage::Merge
    }
  );
  assert!(
    attempt
      .cause()
      .is_some_and(|cause| cause.contains("did not finish in time")),
    "{attempt:?}"
  );
}

/// Git that writes nothing until it is stopped, after two minutes.
struct Quiet;

impl CommandRunner for Quiet {
  fn run(&self, _: &Command) -> Result<CommandOutput, CommandError> {
    Err(CommandError::Silent {
      after: Duration::from_secs(120),
    })
  }
}

#[test]
fn merge_attempts_git_that_wrote_nothing_keeps_how_long_it_was_given() {
  let fixture = Fixture::new(false);
  let programs = Programs::with_runner(Arc::new(Quiet));
  merge_in(fixture.root(), &programs, (fixture.run, None)).expect_err("git wrote nothing");
  let attempt = &attempts(&fixture)[0].attempt;
  assert!(
    matches!(attempt.outcome(), AttemptOutcome::GitStopped { .. }),
    "{attempt:?}"
  );
  assert_eq!(attempt.kind(), Some(CauseKind::Unanswered { seconds: 120 }));
}

/// Git that stops part way through bringing the merge in, after writing the run's `task.txt`, and that then cannot
/// put the file back.
struct StopsPartWay {
  checkout: PathBuf,
  git: ProcessRunner,
}

impl CommandRunner for StopsPartWay {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let args = command.args();
    let failed = |stderr: &str| {
      Ok(CommandOutput {
        code: Some(1),
        stdout: String::new(),
        stderr: stderr.to_owned(),
      })
    };
    if args.iter().any(|arg| arg == "--ff-only") {
      fs::write(self.checkout.join("task.txt"), "delivered task\n").expect("git wrote the file");
      return failed("fatal: stopped part way");
    }
    if args.iter().any(|arg| arg == "checkout-index") {
      return failed("error: task.txt is held by another program");
    }
    self.git.run(command)
  }
}

#[test]
fn merge_attempts_stopped_while_bringing_the_merge_in_name_the_files_left_changed() {
  let fixture = Fixture::new(false);
  let programs = Programs::with_runner(Arc::new(StopsPartWay {
    checkout: fixture.root().to_owned(),
    git: ProcessRunner::new(Duration::from_secs(60)),
  }));
  let refused = merge_in(fixture.root(), &programs, (fixture.run, None)).expect_err("left changed");
  assert!(
    matches!(refused, IntegrationProblem::Merge(MergeError::LeftChanged { .. })),
    "{refused:?}"
  );
  let attempt = &attempts(&fixture)[0].attempt;
  assert_eq!(
    attempt.outcome(),
    &AttemptOutcome::LeftChanged {
      files: vec!["task.txt".to_owned()]
    }
  );
  assert_eq!(attempt.cause(), Some("error: task.txt is held by another program"));
}

#[test]
fn merge_attempts_an_attempt_the_app_left_open_is_recorded_interrupted_by_the_next_merge() {
  let fixture = Fixture::new(false);
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  db.open_merge_attempt(&OpenAttempt {
    run: fixture.run,
    at: Timestamp::from_unix_millis(10),
    asked_by: MergeAsker::Automation,
    via: MergeVia::MergeRun,
    stage: AttemptStage::Merge,
  })
  .expect("an attempt the app left");
  merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("merged");
  let recorded = attempts(&fixture);
  assert_eq!(recorded.len(), 2);
  assert!(recorded[0].attempt.outcome().landed());
  let interrupted = &recorded[1].attempt;
  assert_eq!(
    interrupted.outcome(),
    &AttemptOutcome::Interrupted {
      stage: AttemptStage::Merge
    }
  );
  assert_eq!(
    (interrupted.asked_by(), interrupted.via()),
    (MergeAsker::Automation, MergeVia::MergeRun)
  );
  assert_eq!(
    result_of(interrupted.stages(), AttemptStage::Policy),
    StageResult::Passed
  );
  assert!(db.open_merge_attempts().expect("read").is_empty());
}

#[test]
fn merge_attempts_orchesters_own_retry_is_recorded_with_its_number() {
  let fixture = Fixture::new(false);
  let retry = MergeAsker::Retry {
    number: NonZeroU32::new(3).expect("three"),
  };
  merge_in_as(
    fixture.root(),
    &Programs::processes(),
    (fixture.run, None, None),
    (retry, MergeVia::TaskPage),
  )
  .expect("merged");
  let attempt = &attempts(&fixture)[0].attempt;
  assert_eq!(attempt.asked_by(), retry);
  assert!(attempt.outcome().landed());
}

#[test]
fn merge_attempts_a_record_that_cannot_be_written_never_fails_a_landed_merge() {
  let fixture = Fixture::new(false);
  let raw = rusqlite::Connection::open(fixture.root().join(".orchester/orchester.db")).expect("raw database");
  raw
    .execute_batch(
      "CREATE TRIGGER refuse_attempts BEFORE INSERT ON merge_attempts BEGIN SELECT RAISE(ABORT, 'refused'); END;",
    )
    .expect("attempts cannot be written");
  let merged = merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("the merge stands");
  let db = ProjectDb::open(fixture.root()).expect("db");
  assert_eq!(db.run_merge(fixture.run).expect("read"), Some(merged.merge));
  assert!(attempts(&fixture).is_empty());
}

#[test]
fn merge_attempts_waiting_for_review_tells_a_branch_that_moved_after_the_review() {
  let fixture = Fixture::recorded(false, "workspace-", "task", Some("checkpointed task\n"));
  let programs = Programs::processes();
  let waiting = |fixture: &Fixture| {
    let tasks = ProjectTasks::load(fixture.root(), &programs).expect("the tasks read");
    read::waiting_for_review(&tasks, (fixture.root(), &programs)).expect("read")
  };
  let before = waiting(&fixture);
  assert_eq!(before.len(), 1);
  assert_eq!(before[0].run, fixture.run);
  assert_eq!(before[0].review_in, ReviewIn::TaskPage);
  assert_eq!(before[0].since_review, Some(SinceReview::Unchanged));
  let checkpoint = ProjectDb::open(fixture.root())
    .expect("db")
    .latest_checkpoint(fixture.run)
    .expect("read")
    .expect("the run has a checkpoint");
  for subject in ["after the review", "and once more"] {
    fs::write(fixture.task_root.join("task.txt"), format!("{subject}\n")).expect("edit");
    git(&fixture.task_root, &["commit", "-am", subject]);
  }
  let Some(SinceReview::Moved { reviewed, commits }) = waiting(&fixture).remove(0).since_review else {
    panic!("the branch moved after the review");
  };
  assert_eq!(&reviewed, checkpoint.revision());
  let subjects: Vec<&str> = commits.iter().map(|commit| commit.subject.as_str()).collect();
  assert_eq!(subjects, ["and once more", "after the review"]);
}
