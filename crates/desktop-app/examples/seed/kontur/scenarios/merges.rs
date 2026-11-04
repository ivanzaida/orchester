//! Review › Merges (ORC-362, `Review · redesign`), read from the merge attempts the runs recorded. Quick fix runs merge
//! from their task page; Feature delivery's go through the merge queue. Nothing here moves on by itself: the queue's
//! run waits for approval and no retry waits.
//!
//! `merges` (09 `Os7b3`): a run refused before Git at the integration lock after two attempts; two runs waiting for
//! review, one in the merge queue, one on its task page whose branch moved two commits after the reviewed head
//! (*Changed since your review: 2 commits after …*, as `Row · KON-20` `cd7dN`); three runs merged
//! today, by the person after a refused attempt, by automation and by the merge queue; one merged yesterday and one
//! three days ago.
//!
//! `merges-outcomes` (every row of `Cjprb`): a run not merged for each way an attempt ends that the rows tell —
//! refused before Git, stopped in the merge queue, conflicts, merged by Git but not recorded, Git stopped answering,
//! and a bring-in that stopped part way and left a file changed in the checkout (made by a real merge whose git stops
//! after writing it, so *Put files back* puts it back); a run waiting in the queue with two review comments not sent;
//! two runs merged today. Orchester's own retry waits only in a running app, so no row retries here.
//!
//! `merges-empty` (09c `jhPCL`): the delivering catalogue and no merge attempt.
//!
//! `merges-many`: more merged runs than a page, three a day before today.

use std::{num::NonZeroU32, path::PathBuf, sync::Arc, time::Duration};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  conversation::MessageBody,
  integration::{
    MergeError,
    attempt::{AttemptOutcome, AttemptStage, CauseKind, MergeAsker, MergeAttempt, MergeVia},
  },
  merge_queue::{CheckedOn, QueueState},
  pull_request::feedback::{DiffSide, LineAnchor, LineSpan},
  review::{ReviewThread, ThreadKind},
  run::Run,
  store::MergeStore,
  task::Task,
  time::Timestamp,
};
use orchester_exec::{ProcessRunner, programs::Programs};
use orchester_workspace::tasks::integration::{IntegrationProblem, merge_in};

use super::delivery::{Work, checked_events, enter, passed};
use crate::kontur::{Kontur, SeedError, catalogue::Published, files, run::Drive};

pub(super) type Asked = (MergeAsker, MergeVia);
/// How an attempt ended, and why.
pub(super) type Ended<'a> = (AttemptOutcome, Option<&'a str>);

pub(super) const PERSON: Asked = (MergeAsker::Person, MergeVia::TaskPage);
pub(super) const QUEUE: Asked = (MergeAsker::MergeQueue, MergeVia::MergeQueue);
pub(super) const AUTOMATION: Asked = (MergeAsker::Automation, MergeVia::MergeRun);
/// Why the integration lock refused, as its preflight says it.
const HELD: &str = "another merge or local integration holds this checkout";
const MINUTE: i64 = 60_000;
/// The merged runs of `merges-many`: more than a page of Review › Merges (30).
const MANY: u32 = 34;
const CRITERIA: [&str; 1] = ["The change is covered by tests."];
/// The file of `main` that the run whose bring-in stopped part way changes.
const README: &str = "README.md";

/// `merges`.
pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let (feature, quick) = (&catalogue.feature_delivery, &catalogue.quick_fix);
  merged_before_today(kontur, quick, ("Webhook signing keys", 3), AUTOMATION)?;
  merged_before_today(kontur, quick, ("Dead-letter queue for webhooks", 1), PERSON)?;
  let refused_first = Some((refused(AttemptStage::IntegrationLock), HELD));
  merged_today(
    kontur,
    quick,
    ("Delivery log per endpoint", 420),
    (PERSON, refused_first),
  )?;
  merged_today(
    kontur,
    quick,
    ("Honor Retry-After on 429 and 503", 330),
    (AUTOMATION, None),
  )?;
  merged_today(kontur, quick, ("Retry budget per endpoint", 260), (QUEUE, None))?;
  let (_, held) = ready(
    kontur,
    quick,
    ("Add retry to webhook sender", "refused at the integration lock"),
    200,
  )?;
  for ago in [40, 20] {
    let at = kontur.today_ago(ago);
    record(
      kontur,
      &held,
      (at, PERSON),
      (refused(AttemptStage::IntegrationLock), Some(HELD)),
    )?;
  }
  waiting_in_queue(kontur, feature, "Sign webhook payloads")?;
  let task = queued(kontur, quick, ("Endpoint health page", "changed since the review"))?;
  let at = kontur.today_ago(160);
  let mut drive = Drive::start(kontur, &task, quick, at)?;
  drive.pass_all()?;
  drive
    .commit_past_review(
      "Check the endpoint's TLS too",
      &[("notes/health.md", "Health page: one more check.\n".to_owned())],
    )?
    .commit_past_review(
      "Show when the endpoint last answered",
      &[(
        "notes/health.md",
        "Health page: one more check.\nLast answered at.\n".to_owned(),
      )],
    )?;
  Ok(())
}

/// `merges-outcomes`.
pub fn outcomes(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let (feature, quick) = (&catalogue.feature_delivery, &catalogue.quick_fix);
  merged_today(
    kontur,
    quick,
    ("Honor Retry-After on 429 and 503", 420),
    (AUTOMATION, None),
  )?;
  merged_today(kontur, quick, ("Retry budget per endpoint", 360), (QUEUE, None))?;
  let base = kontur.base.clone();
  let not_merged: [(&str, Asked, Ended<'_>); 5] = [
    (
      "Add retry to webhook sender",
      PERSON,
      (refused(AttemptStage::IntegrationLock), Some(HELD)),
    ),
    (
      "Per-endpoint rate limits",
      QUEUE,
      (
        refused(AttemptStage::ProjectDatabase),
        Some("orchester.db-wal is used by another program"),
      ),
    ),
    (
      "Per-endpoint secrets",
      PERSON,
      (
        AttemptOutcome::Conflicts {
          files: vec![
            "src/webhooks/sender.rs".to_owned(),
            "src/webhooks/settings.rs".to_owned(),
          ],
        },
        None,
      ),
    ),
    (
      "Delivery dashboard",
      PERSON,
      (
        AttemptOutcome::NotRecorded { revision: base },
        Some("the project database stayed busy"),
      ),
    ),
    (
      "Rate limit per endpoint",
      PERSON,
      (
        AttemptOutcome::GitStopped {
          stage: AttemptStage::Merge,
        },
        Some("git merge wrote nothing for 120 s and was stopped"),
      ),
    ),
  ];
  for (place, (title, asked, ended)) in (0_i64..).zip(not_merged) {
    let (_, run) = ready(kontur, quick, (title, "not merged"), 300 - place * 30)?;
    let at = kontur.today_ago(90 - place * 10);
    record(kontur, &run, (at, asked), ended)?;
  }
  left_changed(kontur, quick)?;
  let waiting = waiting_in_queue(kontur, feature, "Sign webhook payloads")?;
  unsent_comments(kontur, &waiting, 2)
}

/// `merges-empty`.
pub fn empty(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.delivering_catalogue().map(drop)
}

/// `merges-many`: more merged runs than Review › Merges reads at first, three a day over the days before today, so
/// the older days are read as its list scrolls.
pub fn many(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let quick = &catalogue.quick_fix;
  for number in 0..MANY {
    let title = format!("Merged change {}", MANY - number);
    merged_before_today(kontur, quick, (&title, 1 + i64::from(number) / 3), PERSON)?;
  }
  Ok(())
}

/// Records the attempt of `run` at `at` that `asked` asked for, ended as `outcome` for `cause`.
pub(super) fn record(
  kontur: &mut Kontur,
  run: &Run,
  at: (Timestamp, Asked),
  ended: Ended<'_>,
) -> Result<(), SeedError> {
  record_kind(kontur, run, at, ended, None)
}

/// [`record`], its cause of `kind`.
pub(super) fn record_kind(
  kontur: &mut Kontur,
  run: &Run,
  (at, asked): (Timestamp, Asked),
  (outcome, cause): Ended<'_>,
  kind: Option<CauseKind>,
) -> Result<(), SeedError> {
  let attempt = MergeAttempt::new((run.id(), at), asked, outcome, cause).with_kind(kind);
  kontur.db.record_merge_attempt(&attempt)?;
  Ok(())
}

pub(super) const fn refused(stage: AttemptStage) -> AttemptOutcome {
  AttemptOutcome::Refused { stage }
}

/// A queued task titled `title`, on `pipeline`, noted as left in `state`.
fn queued(kontur: &mut Kontur, pipeline: &Published, (title, state): (&str, &'static str)) -> Result<Task, SeedError> {
  let mut task = kontur.task((title, state, &CRITERIA), Some(pipeline), kontur.ago(600))?;
  kontur.seeded(&task, state);
  kontur.queue(&mut task, pipeline)?;
  Ok(task)
}

/// A run of a new task titled `title` that passed every step, started `ago` minutes before now (no earlier than
/// today), and waits to merge from its task page.
pub(super) fn ready(
  kontur: &mut Kontur,
  pipeline: &Published,
  (title, state): (&str, &'static str),
  ago: i64,
) -> Result<(Task, Run), SeedError> {
  let task = queued(kontur, pipeline, (title, state))?;
  let at = kontur.today_ago(ago);
  let mut drive = Drive::start(kontur, &task, pipeline, at)?;
  drive.pass_all()?;
  Ok((task, drive.finish()))
}

/// A run titled `title` merged today, started `ago` minutes before now, its merge asked for by `asked`; with `before`,
/// after an attempt that ended so.
fn merged_today(
  kontur: &mut Kontur,
  pipeline: &Published,
  (title, ago): (&str, i64),
  (asked, before): (Asked, Option<(AttemptOutcome, &str)>),
) -> Result<(), SeedError> {
  let ago = ago.min(minutes_into_today(kontur) - 1);
  let run = passed(kontur, pipeline, (title, "merged today"), (Work::Merged, ago))?;
  if let Some((outcome, cause)) = before {
    record(kontur, &run, (kontur.today_ago(ago), asked), (outcome, Some(cause)))?;
  }
  record_landed(kontur, &run, asked)
}

/// A run titled `title` merged `days` days before today, in the evening, its merge asked for by `asked`.
fn merged_before_today(
  kontur: &mut Kontur,
  pipeline: &Published,
  (title, days): (&str, i64),
  asked: Asked,
) -> Result<(), SeedError> {
  // 18:00 that day, which its steps' minutes keep before midnight.
  let ago = minutes_into_today(kontur) + days * 24 * 60 - 18 * 60;
  let run = passed(kontur, pipeline, (title, "merged before today"), (Work::Merged, ago))?;
  record_landed(kontur, &run, asked)
}

/// Records the attempt that landed `run`'s merge, as `asked` asked for it.
pub(super) fn record_landed(kontur: &mut Kontur, run: &Run, asked: Asked) -> Result<(), SeedError> {
  let merge = kontur
    .db
    .run_merge(run.id())?
    .ok_or_else(|| SeedError::Invalid("the run was not merged".to_owned()))?;
  let landed = AttemptOutcome::Merged {
    revision: merge.revision,
    kind: merge.kind,
  };
  record(kontur, run, (merge.merged_at, asked), (landed, None))
}

/// The minutes since today's local midnight.
pub(super) fn minutes_into_today(kontur: &Kontur) -> i64 {
  (kontur.now.unix_millis() - kontur.now.local_day_start().unix_millis()) / MINUTE
}

/// A run of Feature delivery titled `title` waiting for approval in the merge queue, checked at its last checkpoint.
fn waiting_in_queue(kontur: &mut Kontur, feature: &Published, title: &str) -> Result<Run, SeedError> {
  let run = passed(
    kontur,
    feature,
    (title, "waits for approval in the merge queue"),
    (Work::Clean, 150),
  )?;
  let tip = head(&run)?;
  let base = kontur.base.clone();
  let at = kontur.today_ago(60);
  enter(
    kontur,
    &run,
    (QueueState::WaitingForApproval, at),
    |entry| {
      entry.checked = Some(CheckedOn {
        base: base.clone(),
        tip,
        at,
      });
    },
    &checked_events(&base),
  )?;
  Ok(run)
}

/// `run`'s last checkpoint.
fn head(run: &Run) -> Result<orchester_core::revision::Revision, SeedError> {
  run
    .last_checkpoint()
    .map(|checkpoint| checkpoint.revision().clone())
    .ok_or_else(|| SeedError::Invalid("the run committed nothing".to_owned()))
}

/// `count` review comments on the first line of `run`'s work, written and not sent to Implement.
fn unsent_comments(kontur: &mut Kontur, run: &Run, count: u32) -> Result<(), SeedError> {
  let head = head(run)?;
  let invalid = |error: String| SeedError::Invalid(error);
  let line = NonZeroU32::MIN;
  let span = LineSpan::new(line, line).map_err(|error| invalid(error.to_string()))?;
  let excerpt = vec!["Sign webhook payloads: implement so far, attempt 1".to_owned()];
  for number in 1..=count {
    let anchor = LineAnchor::new("notes/work.md", DiffSide::New, span, excerpt.clone())
      .map_err(|error| invalid(error.to_string()))?;
    let body = MessageBody::new(&format!("Comment {number}: say where the signing key rotates."))
      .map_err(|error| invalid(error.to_string()))?;
    let at = kontur.today_ago(30);
    let thread = ReviewThread::start(run.id(), (ThreadKind::Comment, anchor, head.clone()), &body, at);
    kontur.db.add_review_thread(&thread)?;
  }
  Ok(())
}

/// A run of Quick fix that changes the README, whose merge brought it into the checkout and stopped part way, git then
/// unable to put it back: its attempt is recorded as the app records it, and the checkout keeps the run's README.
pub(super) fn left_changed(kontur: &mut Kontur, quick: &Published) -> Result<(), SeedError> {
  let task = queued(kontur, quick, ("Payload size guard", "left a file changed"))?;
  let readme = files::initial()
    .into_iter()
    .find(|(path, _)| *path == README)
    .map(|(_, content)| format!("{content}\nPayloads over 1 MB are refused.\n"))
    .ok_or_else(|| SeedError::Invalid(format!("the repository has no {README}")))?;
  let at = kontur.today_ago(120);
  let mut drive = Drive::start(kontur, &task, quick, at)?;
  drive
    .pass_before("implement")?
    .pass_with("implement", &[(README, readme.clone())])?
    .pass_all()?;
  let run = drive.finish();
  let programs = Programs::with_runner(Arc::new(StopsPartWay {
    file: (kontur.repo.join(README), readme),
    git: ProcessRunner::new(Duration::from_secs(60)),
  }));
  match merge_in(&kontur.repo, &programs, (run.id(), None)) {
    Err(IntegrationProblem::Merge(MergeError::LeftChanged { .. })) => Ok(()),
    Err(problem) => Err(SeedError::Invalid(format!(
      "the merge did not stop part way: {problem:?}"
    ))),
    Ok(_) => Err(SeedError::Invalid("the merge landed".to_owned())),
  }
}

/// Git that stops part way through bringing the merge in, after writing the run's version of a file into the
/// checkout, and that then cannot put the file back.
struct StopsPartWay {
  /// The file, and the run's version of it.
  file: (PathBuf, String),
  git: ProcessRunner,
}

impl CommandRunner for StopsPartWay {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let args = command.args();
    let failed = |stderr: &str| CommandOutput {
      code: Some(1),
      stdout: String::new(),
      stderr: stderr.to_owned(),
    };
    if args.iter().any(|arg| arg == "--ff-only") {
      let wrote = std::fs::write(&self.file.0, &self.file.1);
      return match wrote {
        Ok(()) => Ok(failed("fatal: stopped part way")),
        Err(error) => Err(CommandError::Failed(error.to_string())),
      };
    }
    if args.iter().any(|arg| arg == "checkout-index") {
      return Ok(failed("error: the file is held by another program"));
    }
    self.git.run(command)
  }
}
