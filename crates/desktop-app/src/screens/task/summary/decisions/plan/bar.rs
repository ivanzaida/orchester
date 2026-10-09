//! The decision bar under the Summary's column (`Decision bar` `G5378f`): the note for the step's agent
//! (`Note` `c2mWh`), *Request changes*, which waits for a note, and *Approve plan & continue*; under them
//! what each does, and why the runner refused the last decision. Both decide the plan that waits.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::{Answerer, PlanDecision, ReviewNote};
use orchester_ipc::Command;
use orchester_workspace::tasks::{detail::RunView, plan::PlanReview};

use super::super::{
  action::{DecisionAction, state},
  step_label,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    icon::Icon,
    message::message_text,
  },
};

/// Which decision was asked for, for the button that shows it under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
  Approve,
  RequestChanges,
}

impl lurq::app::component::DevtoolsInspectable for Choice {}

pub struct DecisionBar<'a> {
  pub review: &'a PlanReview,
  pub run: &'a RunView,
  pub note: &'a Signal<String>,
  pub asked: &'a Signal<Option<Choice>>,
  pub action: &'a DecisionAction<Choice>,
}

impl DecisionBar<'_> {
  pub fn render(&self, ctx: &mut Ctx) -> Element {
    let (pending, refused) = state(self.action, self.asked.get());
    let next = self.review.waiting().version.get().saturating_add(1).to_string();
    let note = FieldInput::new("plan-review-note", ctx.t("decisions.plan.noteName"), self.note.clone())
      .leading(Icon::PencilLine)
      .placeholder(ctx.t_args("decisions.plan.notePlaceholder", [("version", next.as_str())]))
      .render(ctx);
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(Column::new().flex(1.0).child(note))
      .child(self.request_changes(ctx, pending))
      .child(self.approve(ctx, pending));
    let hints = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t_args(
          "decisions.plan.changesHint",
          [("agent", &*self.review.agent), ("version", next.as_str())],
        ))
        .id("plan-review-changes-hint")
        .variant(ExtraTypography::Note.role())
        .flex(1.0),
      )
      .child(
        Text::new(&approve_hint(ctx, self.run, self.review))
          .id("plan-review-approve-hint")
          .variant(ExtraTypography::Note.role())
          .nowrap(),
      );
    Column::new()
      .id("plan-review-bar")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Md)
          .bottom(ExtraSpacing::Inset.role())
          .left(SpacingSize::Section)
          .right(SpacingSize::Section),
      )
      .spacing(SpacingSize::Sm)
      .background(PaletteColor::SurfacePanel)
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(row)
      .child(hints)
      .with_children(refused.map(|refused| field_error("plan-review-refused", &message_text(ctx, &refused))))
      .into()
  }

  fn request_changes(&self, ctx: &Ctx, pending: Option<Choice>) -> ActionButton {
    let button = ActionButton::new(
      "plan-request-changes",
      ButtonKind::Secondary,
      ctx.t("decisions.plan.requestChanges"),
    )
    .icon(Icon::Undo2)
    .in_form();
    if pending == Some(Choice::RequestChanges) {
      return button.loading();
    }
    // Asking for changes needs a note saying what should change.
    let Ok(note) = ReviewNote::new(&self.note.get()) else {
      return button.disabled();
    };
    if pending.is_some() {
      return button.disabled();
    }
    let decide = self.decide(Choice::RequestChanges, PlanDecision::ChangesRequested { note });
    button.on_click(move || decide())
  }

  fn approve(&self, ctx: &Ctx, pending: Option<Choice>) -> ActionButton {
    let button = ActionButton::new("plan-approve", ButtonKind::Primary, ctx.t("decisions.plan.approve"))
      .icon(Icon::Check)
      .in_form();
    match pending {
      Some(Choice::Approve) => button.loading(),
      Some(Choice::RequestChanges) => button.disabled(),
      None => {
        let decide = self.decide(Choice::Approve, PlanDecision::Approved);
        button.on_click(move || decide())
      }
    }
  }

  /// Sends `decision` on the plan that waits.
  fn decide(&self, choice: Choice, decision: PlanDecision) -> Arc<dyn Fn() + Send + Sync> {
    let waiting = self.review.waiting();
    let command = Command::DecidePlan {
      run: self.review.run,
      execution: waiting.execution,
      version: waiting.version,
      decision,
      by: Answerer::Person,
    };
    let (action, asked) = (self.action.clone(), self.asked.clone());
    Arc::new(move || {
      asked.set(Some(choice));
      action.run((choice, command.clone()));
    })
  }
}

/// "Approving starts 03 Implement with Builder.": the first step that waits for the gated one, and its
/// agent; or that the run goes on.
fn approve_hint(ctx: &Ctx, run: &RunView, review: &PlanReview) -> Arc<str> {
  let gated = review.waiting().step.clone();
  let next = run
    .run
    .snapshot()
    .body()
    .steps()
    .iter()
    .find(|step| step.depends_on.contains(&gated))
    .and_then(|step| run.steps.iter().find(|view| view.key == step.key));
  match next {
    Some(step) => {
      let name = step_label(ctx, step);
      match step.agent.as_deref() {
        Some(agent) => ctx.t_args("decisions.plan.approveStarts", [("step", &*name), ("agent", agent)]),
        None => ctx.t_args("decisions.plan.approveStartsTool", [("step", &*name)]),
      }
    }
    None => ctx.t("decisions.plan.approveContinues"),
  }
}
