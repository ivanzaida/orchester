//! The answers given on the cards: sent to the project's runner one at a time, on the app's runtime, so an
//! answer under way ends even once no screen waits for it; and what became of each (under way, taken by the
//! runner and when, or why not), which is kept by run and card ([`super::kept`]) and shown on the card until
//! the run is read again.

use std::sync::Arc;

use lurq::{
  app::ctx::{Ctx, FutureAction},
  node::Element,
};
use orchester_core::{id::RunId, time::Timestamp};
use orchester_ipc::{Command, Rejection};
use orchester_workspace::{
  messages::{self, Message},
  runner::RunRequestError,
  tasks::{question::answer_error, run::control_message},
  workspace::OpenProject,
};

use super::{super::frame::note, kept};
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

impl lurq::app::component::DevtoolsInspectable for Given {}

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

/// An answer to send: for the card `key` of `run`, `given` as `command`, to the runner of `project`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Sending {
  pub(super) run: RunId,
  pub(super) key: Arc<str>,
  pub(super) given: Given,
  pub(super) project: OpenProject,
  pub(super) command: Command,
}

pub(super) type SendAnswer = FutureAction<Sending, (), ()>;

/// The action that sends an answer: made once by the tab and asked for in the same place in each render. What
/// became of the answer is kept ([`super::kept`]) where it ends, on the app's runtime, so it is known even
/// once no tab waits for it; the action only says that it ended.
pub(super) fn action(ctx: &mut Ctx) -> SendAnswer {
  let services = Services::of(ctx);
  ctx.future_action(move |sending: Sending| {
    let (runners, runtime, clock) = (
      services.runners.clone(),
      services.runtime.clone(),
      services.clock.clone(),
    );
    async move {
      let Sending {
        run,
        key,
        given,
        project,
        command,
      } = sending;
      let card = key.clone();
      // On the runtime, not in the screen's future: an answer under way ends even once nothing shows it.
      let ended = runtime
        .spawn(async move {
          let stage = match runners.control_run_in(&project, command).await {
            Ok(()) => Stage::Taken(clock.now()),
            Err(error) => Stage::Refused(refused(error)),
          };
          kept::set_outcome((run, &card), Outcome { given, stage });
        })
        .await;
      if let Err(error) = ended {
        tracing::error!(%error, "the answer did not finish");
        let stage = Stage::Refused(Refused::Failed(messages::interrupted()));
        kept::set_outcome((run, &key), Outcome { given, stage });
      }
      Ok(())
    }
  })
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
