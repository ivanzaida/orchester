//! The task conversation at work (`docs/design/orchester-workspace.md`, "Task conversation"). A
//! person's messages, stored by the desktop, are delivered at the run's next invocation boundary:
//! every agent invocation that starts takes the messages queued for its agent into its prompt; an
//! agent that finishes its turn while messages for it are queued does not end its step but continues
//! its session to read them (after its work is committed, like Apply now); a step whose agent asked a
//! question resumes as soon as an answer comes; a run that succeeded is followed up (Ask agent). A
//! turn that ends after messages were delivered acknowledges them, and its final message is the
//! agent's reply. What people wrote while no runner was attached is delivered once one attaches.
//!
//! This is the project's side, for runs no task executes; what a run's task does with its messages is
//! `crate::execution::executor::conversation`'s.
//!
//! [`Command::DeliverMessages`](orchester_ipc::Command) only wakes the run: what is queued is read
//! from the database.

use std::path::Path;

use orchester_core::{
  conversation,
  id::RunId,
  run::{Run, RunStatus},
  store::{ConversationStore, MergeStore},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;

use super::{
  Scheduler, failed, idle,
  link::LocalLink,
  prepare::{self, Prepared},
};
use crate::execution::executor::{control::Control, question::held_by_question};

impl Scheduler {
  /// Delivers the messages queued for `run` at its next invocation boundary: a task executing the
  /// run takes them as its agents reach one; an idle run whose agent waits for an answer resumes,
  /// one that succeeded is followed up, and one that failed or waits for the person's decision
  /// continues with a message written to continue it. Returns once that is arranged; a paused or
  /// stopped run keeps them queued until it continues.
  pub async fn deliver_messages(&self, run: RunId) -> Result<(), Rejection> {
    loop {
      let one_at_a_time = self.inner.continuing.lock().await;
      if self.inner.board.is_stopping() {
        return Err(Rejection::ShuttingDown);
      }
      if let Some(answer) = self.ask(run, Control::DeliverMessages) {
        drop(one_at_a_time);
        if let Ok(result) = answer.await {
          result?;
          let stored = idle::load(&self.open()?, run)?;
          if stored.status().is_terminal() {
            // The task may be finishing just as the message arrives. Wait for its removal,
            // then deliver through the idle path instead of acknowledging an unread message.
            tokio::task::yield_now().await;
            continue;
          }
          return Ok(());
        }
        continue;
      }
      return self.deliver_idle(run);
    }
  }

  pub(super) fn deliver_idle(&self, run: RunId) -> Result<(), Rejection> {
    // No task executes the run, or its task ended without reading the request.
    let mut db = self.open()?;
    let prepared = match deliver_idle(&mut db, &self.inner.project_root, run) {
      Ok(Some(prepared)) => prepared,
      Ok(None) => return Ok(()),
      // The messages stay queued; they are delivered once the blocking tasks finish.
      Err(rejection) if super::blocked_runs::is_blocked(&rejection) => {
        self.hold_for_blockers(run);
        return Err(rejection);
      }
      Err(rejection) => return Err(rejection),
    };
    self.inner.notifier.send(Event::RunChanged {
      run,
      task: prepared.run.task(),
    });
    // Needs no free agent slot: the run waits for one like any other.
    self.spawn(db, prepared)
  }

  /// Executes again those of `waiting`, runs that only wait for a person, whose agent asked a
  /// question while people's messages are queued for the run: written while no runner was
  /// attached, they would otherwise wait for the next message. The run's task resumes the step the
  /// messages reach, or ends again at once.
  pub(super) fn wake_answered(&self, db: &ProjectDb, waiting: Vec<Run>) -> Result<(), Rejection> {
    for run in waiting {
      let id = run.id();
      if self.is_active(run.id()) || !answerable(db, &run)? {
        continue;
      }
      let messages = db
        .messages(run.task())
        .map_err(|error| failed("cannot read the task's conversation", &error))?;
      if !messages.iter().any(|message| message.queued_for(run.id()).is_some()) {
        continue;
      }
      tracing::info!(run = %run.id(), "a question was answered while no runner was attached");
      let prepared = match Prepared::continuing(db, &self.inner.project_root, run) {
        Ok(prepared) => prepared,
        Err(Rejection::RunAgainBlocked {
          block: orchester_core::run::RunAgainBlock::TaskBlocked,
        }) => {
          tracing::info!("answered run remains waiting for its blocking tasks");
          self.hold_for_blockers(id);
          continue;
        }
        Err(rejection) => return Err(rejection),
      };
      self.spawn(self.open()?, prepared)?;
    }
    Ok(())
  }

  /// Follows up the runs that succeeded, and were not merged, while people's messages are queued
  /// for them: written while no runner was attached, they would otherwise wait for the next
  /// message. Each is followed up once, as [`Scheduler::deliver_messages`] does; its messages then
  /// no longer wait. A run that cannot be followed up keeps them queued.
  pub(super) fn follow_up_queued(&self, db: &mut ProjectDb) -> Result<(), Rejection> {
    let runs = db
      .runs_with_queued_messages()
      .map_err(|error| failed("cannot read the queued messages", &error))?;
    for run in runs.into_iter().filter(|&run| !self.is_active(run)) {
      match follow_up_after_attach(db, &self.inner.project_root, run) {
        Ok(Some(prepared)) => {
          tracing::info!(%run, "messages were written to a run ready for review while no runner was attached");
          self.inner.notifier.send(Event::RunChanged {
            run,
            task: prepared.run.task(),
          });
          self.spawn(self.open()?, prepared)?;
        }
        Ok(None) => {}
        Err(rejection) if super::blocked_runs::is_blocked(&rejection) => self.hold_for_blockers(run),
        Err(rejection) => tracing::warn!(%run, ?rejection, "queued messages cannot be delivered; they stay queued"),
      }
    }
    Ok(())
  }
}

/// What delivering messages does to `run` while no task executes it: an ended or merged run takes
/// none, a succeeded one is followed up and executes again, one whose agent waits for an answer executes
/// again to resume it, a failed one or one that waits for the person's decision continues; anything
/// else keeps them queued.
fn deliver_idle(db: &mut ProjectDb, root: &Path, run: RunId) -> Result<Option<Prepared>, Rejection> {
  let stored = idle::load(db, run)?;
  match stored.status() {
    RunStatus::Failed(_) | RunStatus::WaitingForPerson(_) => super::recovery::recover(db, root, stored),
    RunStatus::Cancelled => Err(Rejection::RunEnded),
    // Its work was merged into its base branch: the task is done.
    RunStatus::Succeeded if merged(db, run)? => Err(Rejection::RunEnded),
    RunStatus::Succeeded => follow_up(db, root, stored),
    RunStatus::Running if answerable(db, &stored)? => Prepared::continuing(db, root, stored).map(Some),
    _ => Ok(None),
  }
}

/// Whether a step of `run` waits for an answer that a message can give: one that no question with suggested
/// answers holds.
fn answerable(db: &ProjectDb, run: &Run) -> Result<bool, Rejection> {
  let link = LocalLink::over(db);
  for execution in run.awaiting_answer() {
    if !held_by_question(&link, run.id(), execution)
      .map_err(|error| failed("cannot read the run's questions", &error))?
    {
      return Ok(true);
    }
  }
  Ok(false)
}

/// What attaching does to `run`, for which people's messages are queued: one that succeeded and was
/// not merged is followed up, one that failed or waits for the person's decision continues with a
/// message written to continue it; anything else keeps them queued.
fn follow_up_after_attach(db: &mut ProjectDb, root: &Path, run: RunId) -> Result<Option<Prepared>, Rejection> {
  let stored = idle::load(db, run)?;
  if matches!(stored.status(), RunStatus::Failed(_) | RunStatus::WaitingForPerson(_)) {
    return super::recovery::recover(db, root, stored);
  }
  if stored.status() != &RunStatus::Succeeded || merged(db, run)? {
    return Ok(None);
  }
  follow_up(db, root, stored)
}

fn merged(db: &ProjectDb, run: RunId) -> Result<bool, Rejection> {
  Ok(
    db.run_merge(run)
      .map_err(|error| failed("cannot read the run's merge", &error))?
      .is_some(),
  )
}

/// Follows up `run`, which succeeded, for the oldest message queued for one of its agents; `None`
/// when no queued message reaches one. Its agent continues in the run's worktree, so a run whose
/// worktree was removed is refused and keeps its messages queued.
fn follow_up(db: &mut ProjectDb, root: &Path, mut run: Run) -> Result<Option<Prepared>, Rejection> {
  let messages = db
    .messages(run.task())
    .map_err(|error| failed("cannot read the task's conversation", &error))?;
  let Some(step) = conversation::follow_up_step(&run, &messages) else {
    return Ok(None);
  };
  // Refused before the run changes: a follow-up without its worktree would be recorded as running
  // with no agent at work.
  prepare::existing_folder(db, root, &run)?;
  let events = run.follow_up(&step, Timestamp::now()).map_err(idle::rejection)?;
  let prepared = Prepared::continuing(db, root, run)?;
  idle::store(db, &prepared.run, &events)?;
  tracing::info!(run = %prepared.run.id(), %step, "follow-up started");
  Ok(Some(prepared))
}
