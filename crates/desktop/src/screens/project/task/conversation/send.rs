//! Sending from the Conversation tab. A message is stored once per draft (a retry after an unclear
//! failure stores nothing new), sent now or after the turn, then the runner is told: it gives a
//! message sent now to the agent at work, and delivers the others at the run's next invocation
//! boundary. A message the agent's CLI did not take is queued again for when its turn ends. A
//! message stored but not taken by the runner stays queued, and the composer says so.

use std::path::PathBuf;

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{
  conversation::{Recipient, Timing},
  id::{IdempotencyKey, MessageId, RunId, TaskId},
  run::EventSequence,
};
use orchester_ipc::Command;

use crate::{
  background,
  messages::Message,
  runner::Runners,
  tasks::{
    conversation::{send_after_turn_in, send_in},
    run::control_message,
  },
};

/// A message to store and deliver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Outgoing {
  pub(super) repo_root: PathBuf,
  pub(super) key: IdempotencyKey,
  pub(super) task: TaskId,
  pub(super) run: Option<RunId>,
  pub(super) recovery_after: Option<EventSequence>,
  pub(super) recipient: Recipient,
  pub(super) timing: Timing,
  pub(super) text: String,
}

impl lurq::app::component::DevtoolsInspectable for Outgoing {}

/// A stored message: the runner took it, or it stays queued because the runner could not be told.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Sent {
  Taken,
  Queued(Message),
}

impl lurq::app::component::DevtoolsInspectable for Sent {}

pub(super) type SendAction = FutureAction<Outgoing, Sent, Message>;

fn runners(ctx: &mut Ctx) -> Runners {
  ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens")
}

/// Tells the runner that `run` has a message; what it could not take stays queued.
async fn tell(runners: &Runners, run: RunId) -> Sent {
  match runners.control_run(Command::DeliverMessages { run }).await {
    Ok(()) => Sent::Taken,
    Err(error) => {
      tracing::warn!(%error, "the runner did not take the message");
      Sent::Queued(control_message(&error))
    }
  }
}

pub(super) fn send_action(ctx: &mut Ctx) -> SendAction {
  let runners = runners(ctx);
  ctx.future_action(move |outgoing: Outgoing| {
    let runners = runners.clone();
    async move {
      let Outgoing {
        repo_root,
        key,
        task,
        run,
        recovery_after,
        recipient,
        timing,
        text,
      } = outgoing;
      background::run(move || send_in(&repo_root, key, (task, run, recovery_after), (recipient, timing), &text))
        .await?
        .map_err(|error| {
          tracing::warn!(?error, "the message was not stored");
          error.message()
        })?;
      // Before the task has a run, its first step takes the message.
      let Some(run) = run else {
        return Ok(Sent::Taken);
      };
      Ok(tell(&runners, run).await)
    }
  })
}

/// A message its agent's CLI did not take, to send after the turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AfterTurn {
  pub(super) repo_root: PathBuf,
  pub(super) task: TaskId,
  pub(super) run: RunId,
  pub(super) message: MessageId,
}

impl lurq::app::component::DevtoolsInspectable for AfterTurn {}

pub(super) type AfterTurnAction = FutureAction<AfterTurn, Sent, Message>;

pub(super) fn after_turn_action(ctx: &mut Ctx) -> AfterTurnAction {
  let runners = runners(ctx);
  ctx.future_action(move |request: AfterTurn| {
    let runners = runners.clone();
    async move {
      let AfterTurn {
        repo_root,
        task,
        run,
        message,
      } = request;
      background::run(move || send_after_turn_in(&repo_root, task, message))
        .await?
        .map_err(|error| {
          tracing::warn!(?error, "the message was not queued again");
          error.message()
        })?;
      Ok(tell(&runners, run).await)
    }
  })
}
