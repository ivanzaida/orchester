//! What the composer's presses do, off the UI thread. A message is stored once per draft: its key stays
//! until a store succeeds, so a retry after an unclear failure stores nothing new. Then what the composer's
//! action says follows on the project's runner (`follow`): the runner is told, a paused run resumes, an
//! interrupted one continues, an ended task runs again, or the message waits for the limit to be raised.
//! What the runner does not take stays queued, with why. A plan's changes are a decision on the plan,
//! not a message. A message its agent's CLI did not take is queued again for when its turn ends.

use std::{path::PathBuf, sync::Arc};

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{
  conversation::{Recipient, Timing},
  id::{IdempotencyKey, MessageId, RunId, StepExecutionId, TaskId},
  run::{Answerer, EventSequence, PlanDecision, PlanVersion, ReviewNote},
};
use orchester_ipc::Command;
use orchester_workspace::{
  background,
  messages::Message,
  tasks::{
    conversation::{
      composer::Then,
      follow::{RunnerOf, Sent, Stored, follow, tell},
      send_after_turn_in, send_in,
    },
    run::control_message,
  },
  workspace::OpenProject,
};

use crate::services::Services;

/// A message to store, and what follows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Outgoing {
  pub(super) project: OpenProject,
  pub(super) key: IdempotencyKey,
  pub(super) task: TaskId,
  /// The task's latest run, if it has one.
  pub(super) run: Option<RunId>,
  /// The failure a message that continues the run continues after.
  pub(super) recovery_after: Option<EventSequence>,
  pub(super) recipient: Recipient,
  pub(super) timing: Timing,
  pub(super) then: Then,
  /// The latest run's pipeline, which a refusal to run the task again names.
  pub(super) pipeline: Arc<str>,
  pub(super) text: String,
}

impl lurq::app::component::DevtoolsInspectable for Outgoing {}

pub(super) type SendAction = FutureAction<Outgoing, Sent, Message>;

/// Stores the message, then lets what its action says follow on the project's runner.
pub(super) fn send_action(ctx: &mut Ctx) -> SendAction {
  let services = Services::of(ctx);
  ctx.future_action(move |outgoing: Outgoing| {
    let services = services.clone();
    async move {
      let stored = outgoing.clone();
      // A message for the task's next run names no run: that run's first agent takes it.
      let run = stored.run.filter(|_| !stored.then.for_next_run());
      background::run(move || {
        send_in(
          &stored.project.repo_root,
          stored.key,
          (stored.task, run, stored.recovery_after),
          (stored.recipient, stored.timing),
          &stored.text,
        )
      })
      .await?
      .map_err(|error| {
        tracing::warn!(?error, "the message was not stored");
        error.message()
      })?;
      // Before the task has a run, its first step takes the message.
      let Some(run) = outgoing.run else {
        return Ok(Sent::Taken);
      };
      let stored = Stored {
        root: &outgoing.project.repo_root,
        task: outgoing.task,
        run,
        key: outgoing.key,
        pipeline: &outgoing.pipeline,
      };
      let runner = (
        &services.runners,
        &services.servers,
        RunnerOf::Project(&outgoing.project),
      );
      Ok(follow(runner, &stored, outgoing.then).await)
    }
  })
}

/// *Request changes*: the draft as the changes plan `version` of `run` needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PlanChanges {
  pub(super) project: OpenProject,
  pub(super) run: RunId,
  pub(super) execution: StepExecutionId,
  pub(super) version: PlanVersion,
  pub(super) note: ReviewNote,
}

impl lurq::app::component::DevtoolsInspectable for PlanChanges {}

pub(super) type PlanAction = FutureAction<PlanChanges, (), Message>;

/// Tells the runner the person's decision on the plan: it returns to its agent with the note.
pub(super) fn plan_action(ctx: &mut Ctx) -> PlanAction {
  let runners = Services::of(ctx).runners;
  ctx.future_action(move |changes: PlanChanges| {
    let runners = runners.clone();
    async move {
      let command = Command::DecidePlan {
        run: changes.run,
        execution: changes.execution,
        version: changes.version,
        decision: PlanDecision::ChangesRequested { note: changes.note },
        by: Answerer::Person,
      };
      runners
        .control_run_in(&changes.project, command)
        .await
        .map_err(|error| {
          tracing::warn!(%error, "the runner did not take the plan's changes");
          control_message(&error)
        })
    }
  })
}

/// A message its agent's CLI did not take, to send after the turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AfterTurn {
  pub(super) project: OpenProject,
  pub(super) task: TaskId,
  pub(super) run: RunId,
  pub(super) message: MessageId,
}

impl lurq::app::component::DevtoolsInspectable for AfterTurn {}

pub(super) type AfterTurnAction = FutureAction<AfterTurn, Sent, Message>;

/// Queues the message again for when its agent's turn ends, then tells the runner.
pub(super) fn after_turn_action(ctx: &mut Ctx) -> AfterTurnAction {
  let services = Services::of(ctx);
  ctx.future_action(move |request: AfterTurn| {
    let services = services.clone();
    async move {
      let root: PathBuf = request.project.repo_root.clone();
      let (task, message) = (request.task, request.message);
      background::run(move || send_after_turn_in(&root, task, message))
        .await?
        .map_err(|error| {
          tracing::warn!(?error, "the message was not queued again");
          error.message()
        })?;
      let runner = (
        &services.runners,
        &services.servers,
        RunnerOf::Project(&request.project),
      );
      Ok(tell(runner, (&request.project.repo_root, task, request.run)).await)
    }
  })
}
