//! The top card of a task page whose run waits to be merged (ORC-731; `Review · redesign`, Row 4 `XHQpC`), one
//! scenario per state, each one task, *Add retry to webhook sender*, as the design draws it:
//!
//! - `task-merge-card-ready` (01i `k9GBy`): every step passed, never merged: *Ready to merge* with Merge ⌘↵.
//! - `task-merge-card-refused` (01j `k4C0Es`): the person's merge refused at the integration lock, which another
//!   operation held: *Not merged into main* with Try again ⌘↵.
//! - `task-merge-card-conflicts`: the person's merge stopped at conflicts in two files: *Conflicts with main* with
//!   Resolve ⌘↵.

use orchester_core::integration::attempt::{AttemptOutcome, AttemptStage};

use super::merges::{PERSON, ready, record, refused};
use crate::kontur::{Kontur, SeedError};

/// The design's task.
const TITLE: &str = "Add retry to webhook sender";
/// Why the integration lock refused, as its preflight says it (`PreflightCause::Held`).
const HELD: &str = "held by another operation";
/// When the run started, in minutes before now; its attempt an hour later.
const STARTED: i64 = 120;

pub fn ready_to_merge(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  ready(kontur, &catalogue.quick_fix, (TITLE, "ready to merge"), STARTED)?;
  Ok(())
}

pub fn refused_at_lock(kontur: &mut Kontur) -> Result<(), SeedError> {
  not_merged(
    kontur,
    "merge refused",
    (refused(AttemptStage::IntegrationLock), Some(HELD)),
  )
}

pub fn conflicts(kontur: &mut Kontur) -> Result<(), SeedError> {
  let files = vec![
    "src/webhooks/sender.rs".to_owned(),
    "src/webhooks/settings.rs".to_owned(),
  ];
  not_merged(kontur, "merge conflicted", (AttemptOutcome::Conflicts { files }, None))
}

/// The design's task, its run's one attempt, the person's, ended as `ended`; noted as left in `state`.
fn not_merged(
  kontur: &mut Kontur,
  state: &'static str,
  ended: (AttemptOutcome, Option<&str>),
) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let (_, run) = ready(kontur, &catalogue.quick_fix, (TITLE, state), STARTED)?;
  let at = kontur.today_ago(STARTED - 60);
  record(kontur, &run, (at, PERSON), ended)
}
