//! An agent's tool request in the Orchestrator's conversation, while the Orchestrator takes it up (decided by the
//! person on 2026-11-11; `Orchestrator WIP` `mPw5l`, section `lPhdk`, states `ICzi3`). Its wait's line
//! (`super`) says where it stands: the Orchestrator reads it (a), allowed it once (b) or denied it (c), it waits
//! for the person (d), or the person answered it (e). Under b and c the Orchestrator's answer nests as the answered
//! wait's quote (`Jzqle`): what was asked, in the mono face, then its reason or the words it told the agent
//! ([`quote`]). Under d, past what the Orchestrator said, the request's card is the task's Conversation tab's own (`06b
//! · Command approval` `iw7me`, card `I8MyfC`), framed as there, 8 under and 24 in from the column's start
//! ([`RequestCard`]): answering it here answers the request exactly as there.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{ActionOutcome, ToolAnswer},
  project::TaskKey,
  run::{DecidedBy, ToolApprovalDecision},
};
use orchester_workspace::{
  orchestrator::requests::{ChatRequest, RequestDecided},
  tasks::timeline::approval_key,
};

use super::{
  super::{line::nested, timeline::TurnRow, turns::Part},
  Status,
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  screens::task::cards::Decisions,
  shell::needs_you::in_place::{InPlaceCard, InPlaceProps},
  theme::{CardsText, OrchestratorSpacing},
};

/// The tool whose action is the Orchestrator's answer to a request.
pub(super) const DECIDE_APPROVAL: &str = "decide_approval";

/// The Orchestrator's own answer to the request among `answer`: a `decide_approval` it carried out.
pub(super) fn own_answer(answer: &[Part]) -> Option<&ToolAnswer> {
  answer.iter().find_map(|part| match part {
    Part::Action { action, .. } if action.tool == DECIDE_APPROVAL && action.outcome == ActionOutcome::Done => {
      action.tool_answer.as_ref()
    }
    _ => None,
  })
}

/// The chip's words of how the person answered the request (`decided`); `None` for an answer that is not theirs.
pub(super) const fn by_you(decided: &RequestDecided) -> Option<&'static str> {
  if decided.for_project {
    return Some("orchestrator.wait.status.allowedProjectByYou");
  }
  match (decided.decision, decided.by) {
    (Some(ToolApprovalDecision::Denied), Some(DecidedBy::Person)) => Some("orchestrator.wait.status.deniedByYou"),
    (Some(ToolApprovalDecision::Approved), Some(DecidedBy::Person)) => Some("orchestrator.wait.status.allowedByYou"),
    (_, Some(DecidedBy::PersonSimilar)) => Some("orchestrator.wait.status.allowedSimilarByYou"),
    (_, Some(DecidedBy::PersonRun)) => Some("orchestrator.wait.status.allowedRunByYou"),
    (_, Some(DecidedBy::PersonAlways)) => Some("orchestrator.wait.status.alwaysAllowedByYou"),
    _ => None,
  }
}

/// What the Orchestrator's answer `answer` to `agent`'s request says under its line (b, c): what was asked, in the
/// mono face, then why it allowed it once, or the words it told the agent; `id` names it.
pub(super) fn quote(ctx: &Ctx, id: &str, answer: &ToolAnswer, agent: &str) -> Column {
  let said = match (answer.allowed, &answer.told) {
    (true, _) => ctx.t_args("orchestrator.request.allowedOnce", [("reason", answer.reason.as_str())]),
    (false, Some(told)) => ctx.t_args(
      "orchestrator.request.told",
      [("agent", agent), ("words", told.as_str())],
    ),
    (false, None) => ctx.t_args("orchestrator.request.denied", [("reason", answer.reason.as_str())]),
  };
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(&answer.asked)
        .id(format!("{id}-asked"))
        .variant(CardsText::Command.role())
        .selectable(true)
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Text::new(&said)
        .id(format!("{id}-said"))
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary)
        .selectable(true)
        .width(Dimension::Pct(100.0)),
    )
}

/// What the card of a request that waits for the person reads: the conversation's key for it, its project and task.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RequestCardProps {
  pub(super) id: String,
  pub(super) project: ProjectId,
  /// The task's key, which *Open conversation* opens.
  pub(super) task: Arc<str>,
  pub(super) request: ChatRequest,
}

impl DevtoolsInspectable for RequestCardProps {}

/// The request's card under its line (d), with the decisions it shares with nothing else: the task's Conversation
/// tab's card, read for the request's task and framed as there ([`InPlaceCard`]).
pub(super) struct RequestCard {
  decisions: Decisions,
}

impl Component for RequestCard {
  type Props = RequestCardProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      decisions: Decisions::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RequestCardProps>().clone();
    ctx.provide(self.decisions.clone());
    let Ok(key) = props.task.parse::<TaskKey>() else {
      return Element::new();
    };
    let route = Route::Project(
      props.project,
      ProjectPage::Task(key, TaskView::of(TaskTab::Conversation)),
    );
    let request_id: Arc<str> = props.request.id.as_str().into();
    let conversation = Navigation::of(ctx).to(route);
    let card = ctx.mount_keyed::<InPlaceCard>(
      &format!("{}-card", props.id),
      InPlaceProps {
        id: props.id.clone(),
        project: props.project,
        task: props.request.task,
        key: approval_key(&props.request.id),
        step: None,
        waiting: Arc::from([request_id]),
        gone: None,
        conversation,
        framed: true,
      },
    );
    Column::new()
      .id(props.id)
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .left(OrchestratorSpacing::RequestCard.role())
          .top(SpacingSize::Sm),
      )
      .child(card)
      .into()
  }
}

/// A tool request's line `line` (`id`) in `row`, standing at `status`, with what nests right under it, as a line's
/// own quote does (4 under its 32, `ICzi3`): the Orchestrator's answer (b, c); what its turn said, then the request's
/// card while it waits for the person (d); nothing once the person answered it (e). While its turn is read (a), what
/// it says comes as any wait's answer does. The second element is what follows the opening at the chat's spacing.
pub(in crate::screens::orchestrator) fn opening(
  ctx: &mut Ctx,
  row: &TurnRow,
  (id, line): (&str, Element),
  status: Status,
) -> (Element, Option<Element>) {
  let Some(wait) = row.turn.wait.as_deref() else {
    return (line, None);
  };
  let attached = |line: Element, under: Vec<Element>| -> Element {
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(line)
      .with_children(under)
      .into()
  };
  match status {
    Status::Allowed | Status::Denied => {
      let Some(own) = own_answer(&row.turn.answer) else {
        return (line, None);
      };
      let agent = wait.agent.as_deref().or(wait.step.as_deref()).unwrap_or_default();
      let quoted = quote(ctx, &format!("{id}-quote"), own, agent);
      (attached(line, vec![nested(&format!("{id}-answer"), quoted)]), None)
    }
    Status::ByYou(_) | Status::Gone => (line, None),
    Status::Waits => {
      let said = super::super::answer::nested(ctx, row);
      let card = row.turn.request.as_ref().map(|request| {
        let card = format!("{id}-request");
        let props = RequestCardProps {
          id: card.clone(),
          project: row.env.project,
          task: wait.task.as_str().into(),
          request: request.clone(),
        };
        ctx.mount_keyed::<RequestCard>(&card, props)
      });
      (attached(line, said.into_iter().chain(card).collect()), None)
    }
    Status::Reading | Status::Answered | Status::AnsweredByYou => (line, super::super::answer::nested(ctx, row)),
  }
}
