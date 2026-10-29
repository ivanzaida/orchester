//! A tool request an agent waits on (`Approval card` `gwOaJ`): on `surface_raised` in a `warning` stroke,
//! radius 10, 14 by 16 inside and 12 between its parts: what it asks ([`request`]), its answers
//! ([`answers`]) and what automation may do with it. The answers are the approval dialog's, given here (D6);
//! which a request offers is core's decision (`tasks::approvals::ApprovalChoices`), read with the project's
//! waiting requests. *Deny with a message* puts its editor in place of the answers ([`message`], `D2fdTQ`) until
//! *Back*. While auto mode's reviewer judges it, it is a quiet row until the person chooses to answer it
//! ([`reviewing`]). Once it is answered, or its agent stopped waiting, it is a quiet row that says how, by whom and
//! when ([`decided`]), with a denial's message; as soon as the runner takes the person's answer, before the run is
//! read again.

mod answers;
mod decided;
mod message;
mod request;
mod reviewing;
mod words;

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  core::{ElementRef, Signal},
  node::{Element, padding::Padding},
};
use orchester_core::run::{Answerer, DecidedBy, ToolApprovalDecision};
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
  /// The message editor shows in place of the answers.
  editing: Signal<bool>,
  /// What the person writes to the agent.
  message: Signal<String>,
  field: ElementRef,
  /// The editor just opened: its field takes the focus at the next render.
  focus_field: Arc<AtomicBool>,
  /// The person chose to answer a request auto mode's reviewer is still judging: the whole card shows.
  answer_myself: Signal<bool>,
}

impl Component for ApprovalView {
  type Props = ApprovalProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      once: ElementRef::new(),
      asking: ctx.signal(None),
      editing: ctx.signal(false),
      message: ctx.signal(String::new()),
      field: ElementRef::new(),
      focus_field: Arc::default(),
      answer_myself: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ApprovalProps { place, card } = ctx.props::<ApprovalProps>().clone();
    let decisions = Decisions::of(ctx);
    let focus = decisions.take_focus(&place.key);
    let request = &card.request;
    let id = format!("cards-approval-{}", request.id);
    let outcome = decisions.outcome((place.run(), &place.key));
    if let Some(decided) = decided(&card, outcome.as_ref(), &self.message.get_untracked()) {
      return decided::row(ctx, &id, request, &decided);
    }
    if decisions.lost() {
      return decided::row(ctx, &id, request, &Decided::Lost);
    }
    if card.reviewing && !self.answer_myself.get() {
      return reviewing::row(ctx, &id, request, &self.answer_myself);
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
      editing: &self.editing,
      message: &self.message,
      field: &self.field,
      focus_field: &self.focus_field,
    };
    // A native agent's denial carries no words, so its card never edits one.
    let editing = self.editing.get() && request.native.is_none();
    if editing && self.focus_field.swap(false, Ordering::Relaxed) {
      ctx.focus(&self.field);
    } else if focus && !editing && answering.open.is_some() && !decisions.busy() {
      ctx.focus(&self.once);
    }
    let failed = outcome.and_then(|outcome| match outcome.stage {
      Stage::Refused(refused) => Some(refusal(ctx, &id, &refused, &request.agent)),
      Stage::Sending | Stage::Taken(_) => None,
    });
    let head = request::head(ctx, &id, request);
    let body = request::body(ctx, &id, (&place, request), (shown.as_ref(), reviewing, editing));
    let answers = if editing {
      message::editor(ctx, &answering)
    } else {
      vec![answers::answers(ctx, &answering)]
    };
    let padding = Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role());
    // In the panel its parts are 10 apart, as the item's own (`YDJsC`).
    let gap = if place.in_panel {
      ExtraSpacing::Loose.role()
    } else {
      SpacingSize::Md
    };
    waiting_frame(&place, id.clone(), padding, || head)
      .spacing(gap)
      .with_children(body)
      .with_children(answers)
      .with_children(failed)
      .child(answers::automation(ctx, &id, shown.as_ref()))
      .into()
  }
}

/// How the request was answered: as recorded, or as the runner just took the person's answer or said it no
/// longer waits; `None` while it waits. A denial with a message says the words: as recorded, or as the card sent
/// them (`sent`).
fn decided(card: &ApprovalCard, outcome: Option<&Outcome>, sent: &str) -> Option<Decided> {
  if let (Some(_), Some(at), true) = (card.decided, card.decided_at, card.for_project) {
    return Some(Decided::Project { at });
  }
  if let Some(decision) = card.decided {
    return Some(Decided::Recorded {
      decision,
      by: card.decided_by,
      at: card.decided_at,
      note: card.note.as_ref().map(|note| note.as_str().into()),
    });
  }
  match &outcome?.stage {
    Stage::Taken(at) => {
      let Outcome { given, by, .. } = outcome?;
      let decision = if given.denies() {
        ToolApprovalDecision::Denied
      } else {
        ToolApprovalDecision::Approved
      };
      // The record names a rule of the project as any rule a person made; the card says which it was.
      if (*given, *by) == (Given::ApproveProject, Answerer::Person) {
        return Some(Decided::Project { at: *at });
      }
      // Automation gives only Approve once and Deny, with a message or without ([`Decisions::answer`]).
      let by = match by {
        Answerer::Person => approval_answer(*given).decided_by(),
        Answerer::Automation if given.denies() => DecidedBy::AutomationDeny,
        Answerer::Automation => DecidedBy::Automation,
      };
      Some(Decided::Recorded {
        decision: Some(decision),
        by: Some(by),
        at: Some(*at),
        note: (*given == Given::DenyWithMessage).then(|| sent.trim().into()),
      })
    }
    Stage::Refused(Refused::Ended) => Some(Decided::Recorded {
      decision: None,
      by: None,
      at: None,
      note: None,
    }),
    Stage::Refused(Refused::Answered) => Some(Decided::AnsweredAlready),
    Stage::Sending
    | Stage::Refused(Refused::Superseded | Refused::PersonOnly | Refused::WiderPersonOnly | Refused::Failed(_)) => None,
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
