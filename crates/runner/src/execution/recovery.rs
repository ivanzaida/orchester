//! Only a new message after failure, or after the run began to wait for the person, can recover a
//! run; old queued messages never auto-retry it.

use std::path::Path;

use orchester_core::{
  conversation::Recipient,
  run::{Run, RunEventKind},
  store::{ConversationStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use super::{
  failed, idle,
  prepare::{self, Prepared},
};

pub(super) fn recover(db: &mut ProjectDb, root: &Path, mut run: Run) -> Result<Option<Prepared>, Rejection> {
  let listed = db
    .runs_of_task(run.task())
    .map_err(|error| failed("cannot read task runs", &error))?;
  if !matches!(listed.last(), Some(Ok(latest)) if latest.id() == run.id()) {
    return Err(Rejection::RunEnded);
  }
  let events = db
    .events(run.id())
    .map_err(|error| failed("cannot read failure event", &error))?;
  let Some(ended) = events.iter().rev().find(|event| {
    matches!(
      event.kind(),
      RunEventKind::Failed { .. } | RunEventKind::WaitingForPerson { .. }
    )
  }) else {
    return Err(Rejection::RunEnded);
  };
  let messages = db
    .messages(run.task())
    .map_err(|error| failed("cannot read task messages", &error))?;
  let step = messages
    .iter()
    .filter(|message| {
      message
        .recovery_after()
        .is_some_and(|sequence| sequence >= ended.sequence() && sequence <= run.last_sequence())
    })
    .filter_map(|message| message.queued_for(run.id()).map(|recipient| (recipient, message)))
    .find_map(|(recipient, message)| {
      run
        .recovery_step(match recipient {
          Recipient::Orchestrator => None,
          Recipient::Agent(profile) => Some(profile),
        })
        .map(|step| {
          (
            step,
            message.answerer().expect("queued messages are on the person's side"),
          )
        })
    });
  let Some((step, by)) = step else { return Ok(None) };
  prepare::existing_folder(db, root, &run)?;
  let events = run.recover_as(&step, by, Timestamp::now()).map_err(idle::rejection)?;
  // Validate the pinned task and worktree before changing durable run state.
  let prepared = Prepared::continuing(db, root, run)?;
  idle::store(db, &prepared.run, &events)?;
  Ok(Some(prepared))
}
