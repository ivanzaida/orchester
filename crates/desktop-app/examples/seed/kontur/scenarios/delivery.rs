//! The merge queue (ORC-19, the `Delivery` layer `p9aqZc`), on Feature delivery v3 ending in a Deliver step
//! (approve to merge, merge commits, conflicts to Builder, Check again on the merged result: `git diff --check`, which
//! the QA repository passes). States the queue does not move on by itself, so a QA copy shows them as seeded.
//!
//! `delivery`: a run merged by the queue earlier today (`lvupO`); a run waiting for approval after Check passed on the
//! merged result with `main` as it is now (`wN5Bz`; *Merge into main* merges it); and a run whose conflicts with
//! `main`, which moved on after it started, the Integrate step could not resolve (`hKct7`; *Resolve myself* leaves the
//! merge in its worktree, `fAcWt`).
//!
//! `delivery-blocked`: the checkout of `main` holds an uncommitted change, so the run being delivered is blocked
//! and the run behind it waits (`H7OHPT`); committing or discarding the change lets delivery go on.
//!
//! `delivery-empty`: the delivering catalogue and no run: the empty queue says how each pipeline delivers (`mtrlT`).

use orchester_core::{
  merge_queue::{Blocked, CheckedOn, Delivered, Left, QueueEntry, QueueEvent, QueueState},
  resource::StepKey,
  revision::Revision,
  run::Run,
  store::MergeStore,
  task::Task,
  time::Timestamp,
};

use super::decisions::move_main_on;
use crate::kontur::{Kontur, SeedError, catalogue::Published, files, git, run::Drive};

const CRITERIA: [&str; 2] = ["The change is covered by tests.", "Nothing changes for existing users."];
/// The tool of the pipeline's checks, as the queue names it.
const CHECKS: &str = "Run checks";
/// The files the conflicting run and `main` both change.
const CONFLICTS: [&str; 2] = ["src/webhooks/sender.rs", "src/webhooks/settings.rs"];
/// The tracked file the person changed in their checkout and did not commit.
const DIRTY: &str = "src/webhooks/settings.rs";

/// How a run's work was made, and what became of it before the queue had it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Work {
  Clean,
  /// It writes the webhook retry, which `main` changes too after it started.
  Conflicting,
  /// Merged into `main`, its worktree removed.
  Merged,
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let feature = &catalogue.feature_delivery;
  let merged = passed(
    kontur,
    feature,
    ("Show the delivery log", "merged by the merge queue"),
    (Work::Merged, 240),
  )?;
  merged_by_queue(kontur, &merged)?;
  let waiting = passed(
    kontur,
    feature,
    ("Typed errors per module", "waits for approval in the merge queue"),
    (Work::Clean, 150),
  )?;
  let conflicting = passed(
    kontur,
    feature,
    ("Add retry to webhook sender", "left the merge queue with conflicts"),
    (Work::Conflicting, 120),
  )?;
  let main = move_main_on(kontur)?;
  let checked = waiting
    .last_checkpoint()
    .map(|checkpoint| checkpoint.revision().clone())
    .ok_or_else(|| SeedError::Invalid("the run committed nothing".to_owned()))?;
  let at = kontur.today_ago(20);
  enter(
    kontur,
    &waiting,
    (QueueState::WaitingForApproval, at),
    |entry| {
      entry.checked = Some(CheckedOn {
        base: main.clone(),
        tip: checked,
        at,
      });
    },
    &checked_events(&main),
  )?;
  let files: Vec<String> = CONFLICTS.iter().map(|file| (*file).to_owned()).collect();
  let at = kontur.today_ago(10);
  enter(
    kontur,
    &conflicting,
    (QueueState::Left(Left::Conflicts { files: files.clone() }), at),
    |_| {},
    &[
      QueueEvent::Synced {
        base: main,
        files: files.clone(),
      },
      QueueEvent::IntegrateStarted {
        step: StepKey::new("implement").expect("a valid step key"),
        files: files.clone(),
      },
      QueueEvent::Left(Left::Conflicts { files }),
    ],
  )
}

/// `delivery-blocked`: the run being delivered waits for the person's checkout of `main` to be clean, and the run
/// behind it waits its turn.
pub fn blocked(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let feature = &catalogue.feature_delivery;
  let first = passed(
    kontur,
    feature,
    ("Typed errors per module", "blocked by the checkout"),
    (Work::Clean, 90),
  )?;
  let second = passed(
    kontur,
    feature,
    ("Show the delivery log", "waits behind the blocked run"),
    (Work::Clean, 60),
  )?;
  let dirty = vec![format!("M {DIRTY}")];
  let at = kontur.ago(15);
  enter(
    kontur,
    &first,
    (QueueState::Blocked(Blocked::Uncommitted { files: dirty.clone() }), at),
    // Approved, it was about to merge when the checkout blocked it.
    |entry| entry.approved_at = Some(at),
    &[QueueEvent::Blocked(Blocked::Uncommitted { files: dirty })],
  )?;
  let queued_at = kontur.ago(14);
  enter(kontur, &second, (QueueState::Queued, queued_at), |_| {}, &[])?;
  let initial = files::initial();
  let settings = initial
    .iter()
    .find(|(path, _)| *path == DIRTY)
    .map(|(_, content)| format!("{content}// Trying a shorter timeout locally.\n"))
    .ok_or_else(|| SeedError::Invalid(format!("the repository has no {DIRTY}")))?;
  git::write_files(&kontur.repo, &[(DIRTY, settings)])?;
  Ok(())
}

/// `delivery-empty`: nothing in the queue; the project's pipelines, one of which delivers.
pub fn empty(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.delivering_catalogue().map(drop)
}

/// `overview-quiet` (the Overview's 02 `HWznK`): nothing waits and nothing runs; two runs merged today and two drafts
/// ready to start.
pub fn quiet(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let feature = &catalogue.feature_delivery;
  for (title, ago) in [("Retry webhooks with backoff", 200), ("Show the run's account", 120)] {
    passed(kontur, feature, (title, "merged today"), (Work::Merged, ago))?;
  }
  for title in ["Rotate API keys from settings", "Group exports by month"] {
    let task = kontur.task(
      (title, "a draft ready to start", &CRITERIA),
      Some(feature),
      kontur.ago(60),
    )?;
    kontur.seeded(&task, "a draft ready to start");
  }
  Ok(())
}

/// A run of a new task titled `title` that passed every step, started `ago` minutes before now, its work made as
/// `work` says.
fn passed(
  kontur: &mut Kontur,
  pipeline: &Published,
  (title, state): (&str, &'static str),
  (work, ago): (Work, i64),
) -> Result<Run, SeedError> {
  let task = queued(kontur, pipeline, (title, state))?;
  let at = kontur.ago(ago);
  let mut drive = Drive::start(kontur, &task, pipeline, at)?;
  if work == Work::Conflicting {
    drive
      .pass_before("implement")?
      .pass_with("implement", &files::retry_checkpoint())?;
  }
  drive.pass_all()?;
  if work == Work::Merged {
    drive.merge()?;
  }
  Ok(drive.finish())
}

/// A queued task titled `title`, noted in the seed's report as left in `state`.
fn queued(kontur: &mut Kontur, pipeline: &Published, (title, state): (&str, &'static str)) -> Result<Task, SeedError> {
  let created = kontur.ago(300);
  let mut task = kontur.task((title, state, &CRITERIA), Some(pipeline), created)?;
  kontur.seeded(&task, state);
  kontur.queue(&mut task, pipeline)?;
  Ok(task)
}

/// `run`, merged, as the queue merged it: after Check passed and the person approved, its worktree and code index
/// removed and its branch kept.
fn merged_by_queue(kontur: &mut Kontur, run: &Run) -> Result<(), SeedError> {
  let base = kontur.base.clone();
  let merge = kontur
    .db
    .run_merge(run.id())?
    .ok_or_else(|| SeedError::Invalid("the run was not merged".to_owned()))?;
  // Merged today, so the queue lists it under Today.
  let at = kontur.today_ago(30);
  let mut events = checked_events(&base);
  events.extend([
    QueueEvent::Approved {
      by: orchester_core::run::Answerer::Person,
      after: Some(run.last_sequence().get()),
    },
    QueueEvent::Merged {
      revision: merge.revision.clone(),
      kind: merge.kind,
    },
    QueueEvent::CleanedUp {
      worktree_removed: true,
      branch_deleted: false,
    },
  ]);
  let delivered = Delivered {
    revision: merge.revision,
    kind: merge.kind,
    worktree_removed: true,
    branch_deleted: false,
  };
  enter(
    kontur,
    run,
    (QueueState::Merged(delivered), at),
    |entry| entry.approved_at = Some(at),
    &events,
  )
}

/// What the queue recorded of a run synced with `main` at `base` and checked there.
fn checked_events(base: &Revision) -> Vec<QueueEvent> {
  vec![
    QueueEvent::Synced {
      base: base.clone(),
      files: Vec::new(),
    },
    QueueEvent::CheckStarted { base: base.clone() },
    QueueEvent::CheckPassed {
      base: base.clone(),
      tool: CHECKS.to_owned(),
    },
  ]
}

/// `run` joins the queue as the queue's driver joins it once every step passed, and is left in `state` since `at`,
/// `shaped`, with `events` recorded.
fn enter(
  kontur: &mut Kontur,
  run: &Run,
  (state, at): (QueueState, Timestamp),
  shaped: impl FnOnce(&mut QueueEntry),
  events: &[QueueEvent],
) -> Result<(), SeedError> {
  let finished = kontur
    .db
    .merge_queue_candidates()?
    .into_iter()
    .find_map(|(candidate, _, finished)| (candidate == run.id()).then_some(finished))
    .unwrap_or(at);
  kontur.db.join_merge_queue((run.id(), run.task()), finished, finished)?;
  let mut entry = kontur
    .db
    .merge_queue_entry(run.id())?
    .ok_or_else(|| SeedError::Invalid("the run did not join the merge queue".to_owned()))?;
  entry.state = state;
  entry.since = at;
  shaped(&mut entry);
  if !kontur
    .db
    .save_merge_queue_entry(&entry, &QueueState::Queued, (events, at))?
  {
    return Err(SeedError::Invalid(
      "the merge queue entry changed while it was seeded".to_owned(),
    ));
  }
  Ok(())
}
