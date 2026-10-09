//! The answers given on a tab's cards, kept by the tab rather than by a card, so an answer under way still
//! ends, and is shown, after its card scrolled out of the chat's list. One is sent at a time: while it is
//! under way every card holds its answers. It runs on the app's runtime, so it ends even once no screen
//! waits for it. What became of each, by its card's key ([`Results`]), is what the cards show until the run
//! is read again: under way, taken by the runner and when, or why not.

use std::{collections::BTreeMap, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureAction, FutureState},
  core::Signal,
  node::Element,
};
use orchester_core::time::Timestamp;
use orchester_ipc::{Command, Rejection};
use orchester_workspace::{
  messages::{self, Message},
  runner::RunRequestError,
  tasks::{question::answer_error, run::control_message},
  workspace::OpenProject,
};

use super::super::frame::note;
use crate::{
  services::Services,
  ui::{field::field_error, message::message_text},
};

/// What a person gave on a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum Given {
  ApproveOnce,
  ApproveSimilar,
  ApproveEverything,
  Deny,
  /// The answers to a question with suggested answers.
  Answers,
  ApprovePlan,
}

/// Why an answer did not go through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) enum Refused {
  /// No agent waits for an answer any more: it ended without one.
  Ended,
  /// It was answered already, by someone else or by an earlier press.
  Answered,
  /// The plan no longer waits for a decision: a later version came, or the run went on.
  Superseded,
  Failed(Message),
}

/// Where an answer stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) enum Stage {
  Sending,
  /// The runner took it, at the time it said so.
  Taken(Timestamp),
  Refused(Refused),
}

/// An answer of a card and where it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct Outcome {
  pub(in super::super) given: Given,
  pub(in super::super) stage: Stage,
}

/// The answers given on the tab's cards, by card key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Results(pub(super) BTreeMap<Arc<str>, Outcome>);

impl lurq::app::component::DevtoolsInspectable for Results {}

/// An answer to send: for the card `key`, `given` as `command`, to the runner of `project`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Sending {
  pub(super) key: Arc<str>,
  pub(super) given: Given,
  pub(super) project: OpenProject,
  pub(super) command: Command,
}

/// The card an answer was for, with why it did not go through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Refusal {
  key: Arc<str>,
  refused: Refused,
}

impl lurq::app::component::DevtoolsInspectable for Refusal {}

pub(super) type SendAnswer = FutureAction<Sending, Arc<str>, Refusal>;

/// The action that sends an answer: made once by the tab and asked for in the same place in each render.
pub(super) fn action(ctx: &mut Ctx) -> SendAnswer {
  let services = Services::of(ctx);
  ctx.future_action(move |sending: Sending| {
    let (runners, runtime) = (services.runners.clone(), services.runtime.clone());
    async move {
      let Sending {
        key, project, command, ..
      } = sending;
      // On the runtime, not in the screen's future: an answer under way ends even once nothing shows it.
      let sent = runtime
        .spawn(async move { runners.control_run_in(&project, command).await })
        .await;
      let refused = match sent {
        Ok(Ok(())) => return Ok(key),
        Ok(Err(error)) => refused(error),
        Err(error) => {
          tracing::error!(%error, "the answer did not finish");
          Refused::Failed(messages::interrupted())
        }
      };
      Err(Refusal { key, refused })
    }
  })
}

/// Records where the answer `state` reports stands, once it ended, at `now`.
pub(super) fn record(results: &Signal<Results>, state: &FutureState<Arc<str>, Refusal>, now: Timestamp) {
  let (key, stage) = match (&state.error, &state.data) {
    // A failure keeps the data of the answer before it: the error speaks first.
    (Some(refusal), _) if state.is_rejected() => (refusal.key.clone(), Stage::Refused(refusal.refused.clone())),
    (_, Some(key)) if state.is_fulfilled() => (key.clone(), Stage::Taken(now)),
    _ => return,
  };
  results.update(|results| {
    if let Some(outcome) = results.0.get_mut(&key) {
      outcome.stage = stage;
    }
  });
}

fn refused(error: RunRequestError) -> Refused {
  tracing::warn!(%error, "the runner did not take the answer");
  match error {
    RunRequestError::Rejected(Rejection::ToolApprovalNotAwaited | Rejection::QuestionNotAwaited) => Refused::Ended,
    RunRequestError::Rejected(Rejection::PlanNotAwaited) => Refused::Superseded,
    RunRequestError::Rejected(Rejection::ToolApprovalAnswered | Rejection::QuestionAnswered) => Refused::Answered,
    RunRequestError::Rejected(Rejection::InvalidAnswer { error }) => Refused::Failed(answer_error(&error)),
    error => Refused::Failed(control_message(&error)),
  }
}

/// Why `refused` kept the answer of `id`'s card from going through, `agent` being the one that asked.
pub(in super::super) fn refusal(ctx: &Ctx, id: &str, refused: &Refused, agent: &str) -> Element {
  let id = format!("{id}-refused");
  match refused {
    Refused::Ended => note(&id, &ctx.t_args("cards.refused.ended", [("agent", agent)])).into(),
    Refused::Answered => note(&id, &ctx.t("cards.refused.answered")).into(),
    Refused::Superseded => note(&id, &ctx.t("cards.refused.superseded")).into(),
    Refused::Failed(message) => field_error(&id, &message_text(ctx, message)).into(),
  }
}
