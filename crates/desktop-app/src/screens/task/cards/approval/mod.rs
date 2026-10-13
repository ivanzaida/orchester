//! A tool request an agent waits on (`Approval card` `gwOaJ`): on `surface_raised` in a `warning` stroke,
//! radius 10, 14 by 16 inside and 12 between its parts: what it asks ([`request`]), its answers
//! ([`answers`]) and what automation may do with it. The answers are the approval dialog's, given here (D6);
//! which a request offers is core's decision (`tasks::approvals::ApprovalChoices`), read with the project's
//! waiting requests. Once it is answered, or its agent stopped waiting, it is a quiet row that says how,
//! by whom and when ([`decided`]); as soon as the runner takes the person's answer, before the run is read
//! again.

mod answers;
mod decided;
mod request;
mod words;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  core::{ElementRef, Signal},
  node::{Element, padding::Padding},
};
use orchester_core::run::ToolApprovalDecision;
use orchester_workspace::tasks::{approvals::ShownApproval, timeline::ApprovalCard};

use self::{
  answers::{Answering, approval_answer},
  decided::Decided,
};
use super::{
  Place,
  decisions::{Decisions, Given, Outcome, Refused, Stage, refusal},
  frame::waiting_frame,
};
use crate::{
  project::{Projects, TasksState},
  theme::ExtraSpacing,
};

#[derive(Clone, PartialEq)]
struct ApprovalProps {
  place: Place,
  card: ApprovalCard,
}

impl lurq::app::component::DevtoolsInspectable for ApprovalProps {}

pub(super) fn card(ctx: &mut Ctx, place: Place, card: &ApprovalCard) -> Element {
  ctx.mount_keyed::<ApprovalView>(
    &format!("cards-approval-{}", card.request.id),
    ApprovalProps {
      place,
      card: card.clone(),
    },
  )
}

struct ApprovalView {
  once: ElementRef,
  /// The wider answer the card asks the person to confirm, if any.
  asking: Signal<Option<Given>>,
}

impl Component for ApprovalView {
  type Props = ApprovalProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      once: ElementRef::new(),
      asking: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ApprovalProps { place, card } = ctx.props::<ApprovalProps>().clone();
    let decisions = Decisions::of(ctx);
    let focus = decisions.take_focus(&place.key);
    let request = &card.request;
    let id = format!("cards-approval-{}", request.id);
    let outcome = decisions.outcome((place.run(), &place.key));
    if let Some(decided) = decided(&card, outcome.as_ref()) {
      return decided::row(ctx, &id, request, &decided);
    }
    if decisions.lost() {
      return decided::row(ctx, &id, request, &Decided::Lost);
    }
    let (shown, reviewing) = shown(ctx, &place, &request.id);
    let answering = Answering {
      card: (place.run(), &place.key),
      id: &id,
      request,
      shown: shown.as_ref(),
      run: place.run_number(),
      open: place.open(ctx),
      decisions: &decisions,
      once: &self.once,
      asking: &self.asking,
    };
    if focus && answering.open.is_some() && !decisions.busy() {
      ctx.focus(&self.once);
    }
    let failed = outcome.and_then(|outcome| match outcome.stage {
      Stage::Refused(refused) => Some(refusal(ctx, &id, &refused, &request.agent)),
      Stage::Sending | Stage::Taken(_) => None,
    });
    let head = request::head(ctx, &id, request);
    let body = request::body(ctx, &id, (&place, request), (shown.as_ref(), reviewing));
    let padding = Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role());
    waiting_frame(&place, id.clone(), padding, || head)
      .spacing(SpacingSize::Md)
      .with_children(body)
      .child(answers::answers(ctx, &answering))
      .with_children(failed)
      .child(answers::automation(ctx, &id, shown.as_ref()))
      .into()
  }
}

/// How the request was answered: as recorded, or as the runner just took the person's answer or said it no
/// longer waits; `None` while it waits.
fn decided(card: &ApprovalCard, outcome: Option<&Outcome>) -> Option<Decided> {
  if let Some(decision) = card.decided {
    return Some(Decided::Recorded {
      decision,
      by: card.decided_by,
      at: card.decided_at,
    });
  }
  match &outcome?.stage {
    Stage::Taken(at) => {
      let given = outcome?.given;
      let decision = if given == Given::Deny {
        ToolApprovalDecision::Denied
      } else {
        ToolApprovalDecision::Approved
      };
      Some(Decided::Recorded {
        decision: Some(decision),
        by: Some(approval_answer(given).decided_by()),
        at: Some(*at),
      })
    }
    Stage::Refused(Refused::Ended) => Some(Decided::Recorded {
      decision: None,
      by: None,
      at: None,
    }),
    Stage::Refused(Refused::Answered) => Some(Decided::AnsweredAlready),
    Stage::Sending | Stage::Refused(Refused::Superseded | Refused::Failed(_)) => None,
  }
}

/// The request as the project's waiting requests know it, with what it offers, and whether auto mode's
/// reviewer is judging it (the person may answer all the same); `None` until they are read.
fn shown(ctx: &mut Ctx, place: &Place, id: &str) -> (Option<ShownApproval>, bool) {
  let TasksState::Ready(tasks) = Projects::of(ctx).feed(place.project).tasks.get() else {
    return (None, false);
  };
  let find = |list: &[ShownApproval]| list.iter().find(|shown| shown.approval.id == id).cloned();
  match find(&tasks.reviewing_tool_approvals) {
    Some(reviewing) => (Some(reviewing), true),
    None => (find(&tasks.pending_tool_approvals), false),
  }
}
