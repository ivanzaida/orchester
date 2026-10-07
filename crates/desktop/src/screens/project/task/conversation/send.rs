//! Sending from the Conversation tab. A message is stored once per draft (a retry after an unclear
//! failure stores nothing new), sent now or after the turn, then what the composer's action says
//! follows on the displayed project's runner ([`follow`]). A message the agent's CLI did not take is
//! queued again for when its turn ends. A message stored but not taken by the runner stays queued,
//! and the composer says so.

use std::{path::PathBuf, sync::Arc};

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{
  conversation::{Recipient, Timing},
  id::{IdempotencyKey, MessageId, RunId, TaskId},
  run::EventSequence,
};

pub(super) use crate::tasks::conversation::follow::Sent;
use crate::{
  background,
  messages::Message,
  runner::Runners,
  sharing::feed::ServerFeed,
  tasks::conversation::{
    composer::Then,
    follow::{RunnerOf, Stored, follow, tell},
    send_after_turn_in, send_in,
  },
};

/// A message to store and deliver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Outgoing {
  pub(super) repo_root: PathBuf,
  pub(super) key: IdempotencyKey,
  pub(super) task: TaskId,
  /// The task's latest run, if it has one.
  pub(super) run: Option<RunId>,
  pub(super) recovery_after: Option<EventSequence>,
  pub(super) recipient: Recipient,
  pub(super) timing: Timing,
  pub(super) then: Then,
  /// The pipeline of the latest run, which a refusal to run the task again names.
  pub(super) pipeline: Arc<str>,
  pub(super) text: String,
}

impl lurq::app::component::DevtoolsInspectable for Outgoing {}

pub(super) type SendAction = FutureAction<Outgoing, Sent, Message>;

fn runners(ctx: &mut Ctx) -> Runners {
  ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens")
}

pub(super) fn send_action(ctx: &mut Ctx) -> SendAction {
  let runners = runners(ctx);
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |outgoing: Outgoing| {
    let (runners, servers) = (runners.clone(), servers.clone());
    async move {
      let stored = outgoing.clone();
      // A message for the task's next run names no run: that run's first agent takes it.
      let run = stored.run.filter(|_| !stored.then.for_next_run());
      background::run(move || {
        send_in(
          &stored.repo_root,
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
        root: &outgoing.repo_root,
        task: outgoing.task,
        run,
        key: outgoing.key,
        pipeline: &outgoing.pipeline,
      };
      Ok(follow((&runners, &servers, RunnerOf::Displayed), &stored, outgoing.then).await)
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
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |request: AfterTurn| {
    let (runners, servers) = (runners.clone(), servers.clone());
    async move {
      let AfterTurn {
        repo_root,
        task,
        run,
        message,
      } = request;
      let root = repo_root.clone();
      background::run(move || send_after_turn_in(&repo_root, task, message))
        .await?
        .map_err(|error| {
          tracing::warn!(?error, "the message was not queued again");
          error.message()
        })?;
      Ok(tell((&runners, &servers, RunnerOf::Displayed), (&root, task, run)).await)
    }
  })
}
