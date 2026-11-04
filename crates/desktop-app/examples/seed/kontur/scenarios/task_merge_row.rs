//! `task-merge-row`: the Merge row of a task's properties (ORC-364; `Review · redesign` 01h `fUHdZ` and `Cjprb` ›
//! *Task page · Properties › Merge row*), one task per state, each opened at its task page:
//!
//! - *Add retry to webhook sender* (01h): Ready to merge, the person's last merge refused before Git: *Not merged*.
//! - *Webhook replay endpoint*: Ready to merge, never merged: *Waits for your review*.
//! - *Signed delivery receipts*: a Feature delivery run whose merge the queue refused, back in the queue and blocked
//!   behind the README *Payload size guard* left changed in the checkout, so the queue tries again by itself: *Retrying
//!   · 1 of 3*.
//! - *Rate limit per endpoint*: Git stopped answering at Merge: *Git stopped*.
//! - *Payload size guard*: a bring-in that stopped part way and left the README changed: *Files left changed*.
//! - *Per-endpoint secrets*: conflicts in two files: *Conflicts · 2 files*.
//! - *Delivery dashboard*: merged by Git, not recorded: *Not recorded*.
//! - *Delivery log per endpoint*: merged: *Merged*.
//! - *Export webhooks as CSV*: queued, never run: no Merge row.

use orchester_core::{
  integration::attempt::{AttemptOutcome, AttemptStage},
  merge_queue::{Blocked, QueueState},
};

use super::{
  delivery::{Work, checked_events, enter, passed},
  merges::{PERSON, QUEUE, left_changed, minutes_into_today, ready, record, record_landed, refused},
};
use crate::kontur::{Kontur, SeedError, catalogue::Published};

/// Why the integration lock refused, as its preflight says it (`PreflightCause::Held`).
const HELD: &str = "held by another operation";
const CRITERIA: [&str; 1] = ["The change is covered by tests."];
/// The file *Payload size guard* leaves changed in the checkout, as `git status` lists it.
const README_CHANGED: &str = " M README.md";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let (feature, quick) = (&catalogue.feature_delivery, &catalogue.quick_fix);
  not_merged(
    kontur,
    quick,
    ("Add retry to webhook sender", 200),
    (refused(AttemptStage::IntegrationLock), Some(HELD)),
  )?;
  ready(kontur, quick, ("Webhook replay endpoint", "waits for review"), 190)?;
  not_merged(
    kontur,
    quick,
    ("Rate limit per endpoint", 170),
    (
      AttemptOutcome::GitStopped {
        stage: AttemptStage::Merge,
      },
      Some("git merge wrote nothing for 120 s and was stopped"),
    ),
  )?;
  not_merged(
    kontur,
    quick,
    ("Per-endpoint secrets", 160),
    (
      AttemptOutcome::Conflicts {
        files: vec![
          "src/webhooks/sender.rs".to_owned(),
          "src/webhooks/settings.rs".to_owned(),
        ],
      },
      None,
    ),
  )?;
  let base = kontur.base.clone();
  not_merged(
    kontur,
    quick,
    ("Delivery dashboard", 150),
    (
      AttemptOutcome::NotRecorded { revision: base },
      Some("the project database stayed busy"),
    ),
  )?;
  merged(kontur, quick)?;
  left_changed(kontur, quick)?;
  retrying(kontur, feature)?;
  let task = kontur.task(
    ("Export webhooks as CSV", "queued, never run", &CRITERIA),
    Some(quick),
    kontur.ago(60),
  )?;
  kontur.seeded(&task, "queued, never run");
  Ok(())
}

/// A run titled `title` that passed every step, started `ago` minutes before now, whose one attempt, the person's,
/// ended as `ended` an hour later.
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

/// A run merged today, from its task page.
fn merged(kontur: &mut Kontur, quick: &Published) -> Result<(), SeedError> {
  let ago = 140.min(minutes_into_today(kontur) - 1);
  let run = passed(
    kontur,
    quick,
    ("Delivery log per endpoint", "merged today"),
    (Work::Merged, ago),
  )?;
  record_landed(kontur, &run, PERSON)
}

/// A Feature delivery run whose merge the queue refused at Project database, back in its place in the queue and
/// blocked behind the checkout's uncommitted README, which the queue looks at again by itself.
fn retrying(kontur: &mut Kontur, feature: &Published) -> Result<(), SeedError> {
  let run = passed(
    kontur,
    feature,
    ("Signed delivery receipts", "retried by the merge queue"),
    (Work::Clean, 130),
  )?;
  let at = kontur.today_ago(40);
  let base = kontur.base.clone();
  let blocked = Blocked::Uncommitted {
    files: vec![README_CHANGED.to_owned()],
  };
  enter(
    kontur,
    &run,
    (QueueState::Blocked(blocked), at),
    |_| {},
    &checked_events(&base),
  )?;
  record(
    kontur,
    &run,
    (at, QUEUE),
    (
      refused(AttemptStage::ProjectDatabase),
      Some("orchester.db-wal is used by another program"),
    ),
  )
}
