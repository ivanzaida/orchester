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
  app::{
    component::Component,
    ctx::{Ctx, FutureState},
    theme::SpacingSize,
  },
  core::{ElementRef, Signal},
  node::{Element, padding::Padding},
};
use orchester_core::run::ToolApprovalDecision;
use orchester_workspace::tasks::{approvals::ShownApproval, timeline::ApprovalCard};

use self::{
  answers::{Answer, Answering},
  decided::Decided,
};
use super::{
  Place,
  action::{Refused, decide, refusal},
  decisions::Decisions,
  frame::frame,
};
use crate::{
  project::{Projects, TasksState},
  services::Services,
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
  pressed: Signal<Option<Answer>>,
  once: ElementRef,
}

impl Component for ApprovalView {
  type Props = ApprovalProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      pressed: ctx.signal(None),
      once: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ApprovalProps { place, card } = ctx.props::<ApprovalProps>().clone();
    let action = decide(ctx, place.project);
    let request = &card.request;
    let id = format!("cards-approval-{}", request.id);
    let state = action.state().get();
    if let Some(decided) = self.decided(ctx, &card, &state) {
      return decided::row(ctx, &id, request, &decided);
    }
    let (shown, reviewing) = shown(ctx, &place, &request.id);
    if !state.is_pending() && Decisions::of(ctx).is_some_and(|decisions| decisions.take_focus(request.requested_at)) {
      ctx.focus(&self.once);
    }
    let answering = Answering {
      id: &id,
      request,
      shown: shown.as_ref(),
      run: place.run_number(),
      action: &action,
      pressed: &self.pressed,
      once: &self.once,
    };
    let failed = state
      .error
      .as_ref()
      .filter(|_| !state.is_pending())
      .map(|refused| refusal(ctx, &id, refused, &request.agent));
    let head = request::head(ctx, &id, request);
    let body = request::body(ctx, &id, (&place, request), (shown.as_ref(), reviewing));
    frame(id.clone(), true)
      .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
      .spacing(SpacingSize::Md)
      .child(head)
      .with_children(body)
      .child(answers::answers(ctx, &answering))
      .with_children(failed)
      .child(answers::automation(ctx, &id, shown.as_ref()))
      .into()
  }
}

impl ApprovalView {
  /// How the request was answered: as recorded, or as the runner just took the person's answer or said it
  /// no longer waits; `None` while it waits.
  fn decided(&self, ctx: &mut Ctx, card: &ApprovalCard, state: &FutureState<(), Refused>) -> Option<Decided> {
    let now = Services::of(ctx).clock.now();
    if let Some(decision) = card.decided {
      return Some(Decided::Recorded {
        decision,
        by: card.decided_by,
        at: card.decided_at.unwrap_or(now),
      });
    }
    if state.is_pending() {
      return None;
    }
    match (&state.data, &state.error) {
      (Some(()), _) => self.pressed.get().map(|answer| Decided::Recorded {
        decision: Some(if answer == Answer::Deny {
          ToolApprovalDecision::Denied
        } else {
          ToolApprovalDecision::Approved
        }),
        by: Some(answer.answer().decided_by()),
        at: now,
      }),
      (None, Some(Refused::Ended)) => Some(Decided::Recorded {
        decision: None,
        by: None,
        at: now,
      }),
      (None, Some(Refused::Answered)) => Some(Decided::AnsweredAlready),
      (None, Some(Refused::Failed(_)) | None) => None,
    }
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
