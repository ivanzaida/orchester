//! `merge-attempts`: a run's merge attempt page (ORC-363, `Review · redesign` 09b to 09l), one run per state, each
//! opened at `/project/<id>/tasks/<KEY>/merge/<run>`:
//!
//! - *Add retry to webhook sender* (09b `PVZKL`): automation's attempt through `merge_run` refused at Project database,
//!   then the person's refused before Git at the integration lock, held by another operation. Its live retry (09j
//!   `GhnHQ`, the *Retrying* row of `Cjprb`): with the project's tab active, the debug tool `hold_integration` `held:
//!   true` takes the integration lease as another operation would, so *Try again* is refused at the integration lock
//!   and Orchester really tries again by itself — *Retrying by itself · attempt 1 of 3 in 10 s*, then 2 and 3 of 3, 30
//!   s each, with *Try now* and *Stop retrying*, and the same in its Merges row — until the third try ends it Not
//!   merged; `held: false` lets the lease go, so the next try (or *Try now*) merges.
//! - *Per-endpoint rate limits* (09e `s1jIpI`): a Feature delivery run the merge queue stopped, its attempt refused at
//!   Project database; *Try again* puts it back in its place in the queue.
//! - *Per-endpoint secrets* (09f `Q6yPN8`): conflicts in two files.
//! - *Delivery dashboard* (09g `i2rVv`): merged by Git, not recorded; the commit is in `main`, so *Record merge*
//!   records it.
//! - *Delivery log per endpoint* (09h `rzo3w`): merged on its third attempt, after automation's and the person's were
//!   refused.
//! - *Retry budget per endpoint* (09j's attempts): the person's attempt and Orchester's retry 1 of 3 ten seconds later,
//!   both refused at Project database; it shows Not merged with its retry among its attempts.
//! - *Rate limit per endpoint* (09k `X3DJKY`): Git stopped answering at Merge.
//! - *Payload size guard* (09l `Asu5i`): a bring-in that stopped part way and left the README changed in the checkout.

use std::num::NonZeroU32;

use orchester_core::{
  integration::attempt::{AttemptOutcome, AttemptStage, CauseKind, MergeAsker, MergeVia, OsCause},
  merge_queue::{Left, QueueState},
  run::Run,
  time::Timestamp,
};

use super::{
  delivery::{Work, checked_events, enter, passed},
  merges::{
    AUTOMATION, Asked, Ended, PERSON, QUEUE, left_changed, minutes_into_today, ready, record_kind, record_landed,
    refused,
  },
};
use crate::kontur::{Kontur, SeedError, catalogue::Published};

/// Why the integration lock refused, as its preflight says it (`PreflightCause::Held`).
const HELD: &str = "held by another operation";
/// What the OS said of the project database's write-ahead log another program held open.
const IN_USE: &str = ".orchester/orchester.db-wal: The process cannot access the file because it is being used by another process. (OS error 32)";
const GIT_STOPPED: &str = "Git stopped answering while it merged: `git merge --no-ff --no-edit orchester/kon-run` wrote nothing for 120 s and was stopped, and the temporary worktree was removed";
/// How long Git wrote nothing in [`GIT_STOPPED`].
const SILENT_SECONDS: u64 = 120;

/// Records the attempt of `run` at `at` that `asked` asked for, ended as `ended`, with the kind of its cause as the app
/// keeps it.
fn record(kontur: &mut Kontur, run: &Run, at: (Timestamp, Asked), ended: Ended<'_>) -> Result<(), SeedError> {
  let kind = match ended.1 {
    Some(HELD) => Some(CauseKind::HeldByOperation),
    Some(IN_USE) => Some(CauseKind::Os {
      what: OsCause::UsedByAnotherProgram,
      code: Some(32),
    }),
    Some(GIT_STOPPED) => Some(CauseKind::Unanswered {
      seconds: SILENT_SECONDS,
    }),
    _ => None,
  };
  record_kind(kontur, run, at, ended, kind)
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let (feature, quick) = (&catalogue.feature_delivery, &catalogue.quick_fix);
  held(kontur, quick)?;
  queue_stopped(kontur, feature)?;
  not_merged(
    kontur,
    quick,
    ("Per-endpoint secrets", 170),
    (
      AttemptOutcome::Conflicts {
        files: vec![
          "src/webhooks/retry.rs".to_owned(),
          "src/webhooks/delivery.rs".to_owned(),
        ],
      },
      None,
    ),
  )?;
  let base = kontur.base.clone();
  not_merged(
    kontur,
    quick,
    ("Delivery dashboard", 160),
    (
      AttemptOutcome::NotRecorded { revision: base },
      Some("database is locked: the project database stayed busy for 5 s"),
    ),
  )?;
  merged_after_refusals(kontur, quick)?;
  retried(kontur, quick)?;
  not_merged(
    kontur,
    quick,
    ("Rate limit per endpoint", 130),
    (
      AttemptOutcome::GitStopped {
        stage: AttemptStage::Merge,
      },
      Some(GIT_STOPPED),
    ),
  )?;
  left_changed(kontur, quick)
}

/// 09b: automation's attempt refused at Project database, then the person's at the integration lock.
fn held(kontur: &mut Kontur, quick: &Published) -> Result<(), SeedError> {
  let (_, run) = ready(
    kontur,
    quick,
    ("Add retry to webhook sender", "refused before Git"),
    200,
  )?;
  let at = kontur.today_ago(23);
  record(
    kontur,
    &run,
    (at, AUTOMATION),
    (refused(AttemptStage::ProjectDatabase), Some(IN_USE)),
  )?;
  let at = kontur.today_ago(20);
  record(
    kontur,
    &run,
    (at, PERSON),
    (refused(AttemptStage::IntegrationLock), Some(HELD)),
  )
}

/// 09e: a Feature delivery run the merge queue stopped at Project database.
fn queue_stopped(kontur: &mut Kontur, feature: &Published) -> Result<(), SeedError> {
  let run = passed(
    kontur,
    feature,
    ("Per-endpoint rate limits", "stopped in the merge queue"),
    (Work::Clean, 190),
  )?;
  let at = kontur.today_ago(60);
  let base = kontur.base.clone();
  let detail = format!("project_database: {IN_USE}");
  enter(
    kontur,
    &run,
    (QueueState::Left(Left::Stopped { detail }), at),
    |_| {},
    &checked_events(&base),
  )?;
  record(
    kontur,
    &run,
    (at, QUEUE),
    (refused(AttemptStage::ProjectDatabase), Some(IN_USE)),
  )
}

/// A run titled `title` that passed every step, started `ago` minutes before now, whose one attempt, the person's,
/// ended as `ended` a little later.
fn not_merged(
  kontur: &mut Kontur,
  quick: &Published,
  (title, ago): (&str, i64),
  ended: (AttemptOutcome, Option<&str>),
) -> Result<(), SeedError> {
  let (_, run) = ready(kontur, quick, (title, "not merged"), ago)?;
  let at = kontur.today_ago(ago - 60);
  record(kontur, &run, (at, PERSON), ended)
}

/// 09h: merged on the third attempt, after automation's and the person's were refused.
fn merged_after_refusals(kontur: &mut Kontur, quick: &Published) -> Result<(), SeedError> {
  let ago = 150.min(minutes_into_today(kontur) - 1);
  let run = passed(
    kontur,
    quick,
    ("Delivery log per endpoint", "merged after refusals"),
    (Work::Merged, ago),
  )?;
  let first = kontur.today_ago(ago);
  record(
    kontur,
    &run,
    (first, AUTOMATION),
    (refused(AttemptStage::ProjectDatabase), Some(IN_USE)),
  )?;
  let second = Timestamp::from_unix_millis(first.unix_millis() + 3 * 60_000);
  record(
    kontur,
    &run,
    (second, PERSON),
    (refused(AttemptStage::IntegrationLock), Some(HELD)),
  )?;
  record_landed(kontur, &run, PERSON)
}

/// 09j: the person's attempt and Orchester's first retry ten seconds later, both refused at Project database.
fn retried(kontur: &mut Kontur, quick: &Published) -> Result<(), SeedError> {
  let (_, run) = ready(
    kontur,
    quick,
    ("Retry budget per endpoint", "retried by Orchester"),
    140,
  )?;
  // At the start of its minute, so that the retry ten seconds later is in the same minute (12:41:00, 12:41:10).
  let at = kontur.today_ago(30).unix_millis();
  let at = Timestamp::from_unix_millis(at - at.rem_euclid(60_000));
  record(
    kontur,
    &run,
    (at, PERSON),
    (refused(AttemptStage::ProjectDatabase), Some(IN_USE)),
  )?;
  let retry = (
    MergeAsker::Retry {
      number: NonZeroU32::MIN,
    },
    MergeVia::TaskPage,
  );
  let later = Timestamp::from_unix_millis(at.unix_millis() + 10_000);
  record(
    kontur,
    &run,
    (later, retry),
    (refused(AttemptStage::ProjectDatabase), Some(IN_USE)),
  )
}
