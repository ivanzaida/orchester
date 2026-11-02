//! `conversation-context-plenty`, `-near-full` and `-full`: the `tasks` scenario with a reading of how full the
//! Builder's session is on Implement, the step at work in KON-13's run, where it waits for the person's answer to its
//! question (`Task page · redesign` `C3wC8`: `bhtch`, the states and the popover `yCk2o` in `bxHFm`). A run that waits
//! for a person stays so in a QA copy with a runner; one whose agent works ends as interrupted there. The meter shows
//! the reading on Implement's divider and in the composer's bar: 38% (76k of 200k), 86% (172k of 200k) and *Full*
//! (200k of 200k), each of a window the CLI reported.

use orchester_core::{
  connection::context_use::ContextUse,
  store::{ActivityStore, RunStore},
};

use super::tasks;
use crate::kontur::{Kontur, SeedError};

/// The window of the Builder's model, as its CLI reports it.
const WINDOW: u64 = 200_000;

pub fn plenty(kontur: &mut Kontur) -> Result<(), SeedError> {
  with_reading(kontur, ContextUse::reported(76_000, WINDOW))
}

pub fn near_full(kontur: &mut Kontur) -> Result<(), SeedError> {
  with_reading(kontur, ContextUse::reported(172_000, WINDOW))
}

pub fn full(kontur: &mut Kontur) -> Result<(), SeedError> {
  with_reading(kontur, ContextUse::reported(WINDOW, WINDOW))
}

/// The `tasks` project, the latest invocation of KON-13's latest run, the Builder's on Implement, keeping `reading` as
/// its latest, taken a minute ago.
fn with_reading(kontur: &mut Kontur, reading: ContextUse) -> Result<(), SeedError> {
  tasks::project(kontur)?;
  let task = kontur.find("KON-13")?;
  let run = kontur
    .db
    .runs_of_task(task.id())?
    .into_iter()
    .filter_map(Result::ok)
    .next_back()
    .ok_or(SeedError::NotAtWork)?;
  let invocation = kontur
    .db
    .invocations(run.id())?
    .into_iter()
    .next_back()
    .ok_or(SeedError::NotAtWork)?;
  let at = kontur.ago(1);
  kontur.db.save_invocation_context(invocation.id, &reading, at)?;
  Ok(())
}
