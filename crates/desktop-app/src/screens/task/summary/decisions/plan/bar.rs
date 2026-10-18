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
use orchester_core::run::{PlanDecision, ReviewNote};
use orchester_ipc::Command;
use orchester_workspace::{
  decisions::DecisionKind,
  messages::Message,
  tasks::{detail::RunView, plan::PlanReview},
};

use super::super::{
  action::{DecisionAction, state},
  step_label,
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, answerer},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    icon::Icon,
    message::message_text,
    unavailable::reason_line,
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
  pub gate: Option<InputGate>,
  pub person_only: &'a Signal<bool>,
  /// The project's runner is online: deciding needs it.
  pub online: bool,
  /// The card shows an earlier version than the one that waits.
  pub earlier: bool,
}

impl DecisionBar<'_> {
  pub fn render(&self, ctx: &mut Ctx) -> Element {
    let (pending, refused) = state(self.action, self.asked.get());
    let offline = (!self.online).then(|| ctx.t("task.controls.offline"));
    // A note goes only with *Request changes*: while one is typed, approving would drop it. Approving
    // decides the version that waits, so it waits until that one is shown.
    let noted = !self.note.get().trim().is_empty();
    let held = noted || self.earlier;
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
      .child(self.request_changes(ctx, pending, offline.clone()))
      .child(self.approve(ctx, (pending, offline.clone()), held));
    let approve_hint = if self.earlier {
      let version = self.review.waiting().version.to_string();
      ctx.t_args("decisions.plan.switchToApprove", [("version", version.as_str())])
    } else if noted {
      ctx.t("decisions.plan.clearToApprove")
    } else {
      approve_hint(ctx, self.run, self.review)
    };
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
        Text::new(&approve_hint)
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
      .with_children(self.person_only.get().then(|| {
        field_error(
          "plan-review-person-only",
          &message_text(ctx, &Message::new("appMcp.errors.automationNotAllowed")),
        )
      }))
      .with_children(offline.map(|reason| reason_line("plan-review-offline", &reason)))
      .with_children(refused.map(|refused| field_error("plan-review-refused", &message_text(ctx, &refused))))
      .into()
  }

  fn request_changes(&self, ctx: &Ctx, pending: Option<Choice>, offline: Option<Arc<str>>) -> ActionButton {
    let button = ActionButton::new(
      "plan-request-changes",
      ButtonKind::Secondary,
      ctx.t("decisions.plan.requestChanges"),
    )
    .icon(Icon::Undo2)
    .in_form();
    if let Some(reason) = offline {
      return button.unavailable(reason);
    }
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

  /// *Approve plan & continue*; `held` while a note is typed or an earlier version is shown.
  fn approve(&self, ctx: &Ctx, (pending, offline): (Option<Choice>, Option<Arc<str>>), held: bool) -> ActionButton {
    let button = ActionButton::new("plan-approve", ButtonKind::Primary, ctx.t("decisions.plan.approve"))
      .icon(Icon::Check)
      .in_form();
    match (offline, pending) {
      (Some(reason), _) => button.unavailable(reason),
      (None, Some(Choice::Approve)) => button.loading(),
      (None, Some(Choice::RequestChanges)) => button.disabled(),
      (None, None) if held => button.disabled(),
      (None, None) => {
        let decide = self.decide(Choice::Approve, PlanDecision::Approved);
        button.on_click(move || decide())
      }
    }
  }

  /// Sends `decision` on the plan that waits.
  fn decide(&self, choice: Choice, decision: PlanDecision) -> Arc<dyn Fn() + Send + Sync> {
    let waiting = self.review.waiting();
    let (run, execution, version) = (self.review.run, waiting.execution, waiting.version);
    let (gate, person_only) = (self.gate.clone(), self.person_only.clone());
    let (action, asked) = (self.action.clone(), self.asked.clone());
    Arc::new(move || {
      // A second press while the first is under way would replace it (lurq's `FutureAction::run`).
      if action.state().get_untracked().is_pending() {
        return;
      }
      let Some(by) = answerer(gate.as_ref(), Guarded::Decision(DecisionKind::Plan)) else {
        person_only.set(true);
        return;
      };
      person_only.set(false);
      asked.set(Some(choice));
      action.run((
        choice,
        Command::DecidePlan {
          run,
          execution,
          version,
          decision: decision.clone(),
          by,
        },
      ));
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
