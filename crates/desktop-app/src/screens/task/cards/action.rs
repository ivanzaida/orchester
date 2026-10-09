//! A person's decision on a card, sent to the project's runner, and why it did not go through: the request
//! no longer waits (its agent ended), someone answered it first, or anything else in the runner's words.

use lurq::{
  app::ctx::{Ctx, FutureAction},
  node::Element,
};
use orchester_core::id::ProjectId;
use orchester_ipc::{Command, Rejection};
use orchester_workspace::{
  messages::Message,
  runner::{RequestError, RunRequestError},
  tasks::{question::answer_error, run::control_message},
};

use super::frame::note;
use crate::{
  project::Projects,
  services::Services,
  ui::{field::field_error, message::message_text},
};

/// Why a decision did not go through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Refused {
  /// No agent waits for an answer any more: it ended without one.
  Ended,
  /// It was answered already, by someone else or by an earlier press.
  Answered,
  Failed(Message),
}

impl lurq::app::component::DevtoolsInspectable for Refused {}

pub(super) type Decide = FutureAction<Command, (), Refused>;

/// The action that sends a decision of a card of `project` to the project's runner; a component calls it
/// on every render, in the same place.
pub(super) fn decide(ctx: &mut Ctx, project: ProjectId) -> Decide {
  let runners = Services::of(ctx).runners;
  let projects = Projects::of(ctx);
  ctx.future_action(move |command: Command| {
    let runners = runners.clone();
    let open = projects.state_now(project).and_then(|state| state.open().cloned());
    async move {
      let Some(open) = open else {
        // The project's runner is gone with it: the runner's own words for that.
        return Err(Refused::Failed(control_message(&RunRequestError::Request(
          RequestError::NotConnected,
        ))));
      };
      runners.control_run_in(&open, command).await.map_err(refused)
    }
  })
}

fn refused(error: RunRequestError) -> Refused {
  tracing::warn!(%error, "the runner did not take the decision");
  match error {
    RunRequestError::Rejected(
      Rejection::ToolApprovalNotAwaited | Rejection::QuestionNotAwaited | Rejection::PlanNotAwaited,
    ) => Refused::Ended,
    RunRequestError::Rejected(Rejection::ToolApprovalAnswered | Rejection::QuestionAnswered) => Refused::Answered,
    RunRequestError::Rejected(Rejection::InvalidAnswer { error }) => Refused::Failed(answer_error(&error)),
    error => Refused::Failed(control_message(&error)),
  }
}

/// Why `refused` kept the decision of `id`'s card from going through, `agent` being the one that asked.
pub(super) fn refusal(ctx: &Ctx, id: &str, refused: &Refused, agent: &str) -> Element {
  let id = format!("{id}-refused");
  match refused {
    Refused::Ended => note(&id, &ctx.t_args("cards.refused.ended", [("agent", agent)])).into(),
    Refused::Answered => note(&id, &ctx.t("cards.refused.answered")).into(),
    Refused::Failed(message) => field_error(&id, &message_text(ctx, message)).into(),
  }
}
