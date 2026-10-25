//! The card at the top of the Summary of a run that passed every step and committed nothing (ORC-93): the Ready card
//! with *Complete task* for a run whose result is its report or work done outside the repository (`hyzq2`); the
//! warning that the run committed nothing although its pipeline expects changes, with *Complete anyway*, *Return to a
//! step* (which brings the return card beside the Summary into view) and *Run again* (which opens Run again's
//! confirmation under the header) (`iw9rS`); and once completed, who completed it and when (`nqp68`).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, events::is_synthetic_input, theme::SpacingSize},
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::IdempotencyKey,
  resource::Deliverable,
  run::{Answerer, CompletedBy, RunCompletion},
};
use orchester_workspace::{
  decisions::DecisionKind,
  display,
  tasks::{completion::RunResult, detail::RunView, run_again::block_message},
};

use super::CompleteAction;
use crate::{
  mcp::input_gate::{Guarded, InputGate, answerer},
  navigation::{
    Navigation,
    pointed::{Pointed, PointedCard},
    press::OpenIn,
  },
  routes::{Route, TaskTab, TaskView},
  screens::task::{
    PageState, TaskContext,
    header::run_again::{self, Confirming},
  },
  services::Services,
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    unavailable::reason_line,
  },
};

/// The person's leave for window input, which *Complete anyway* needs as any decision on a run that waits for them.
pub struct Gated<'a> {
  pub gate: Option<&'a InputGate>,
  /// Window input pressed *Complete anyway* without it.
  pub person_only: &'a Signal<bool>,
}

/// The Ready card of a run whose result is its report: *Ready to complete*, with *Complete task*.
pub fn ready(
  ctx: &mut Ctx,
  (task, run, result): (&TaskContext, &RunView, &RunResult),
  action: &CompleteAction,
) -> Element {
  let body = match result.delivers {
    Deliverable::Outside => "decisions.complete.outsideBody",
    Deliverable::Report | Deliverable::ChangesOrReport | Deliverable::Changes => "decisions.complete.reportBody",
  };
  let button = ActionButton::new(
    "task-complete",
    ButtonKind::Primary,
    ctx.t("decisions.complete.complete"),
  )
  .icon(Icon::CircleCheck);
  let button = pressed(button, (task, run), action, None);
  let actions = Row::new()
    .id("task-complete-actions")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(button);
  let reasons = refused(ctx, action).into_iter().collect();
  super::super::ready::frame(
    (ctx.t("decisions.complete.title"), ctx.t(body)),
    actions.into(),
    reasons,
  )
}

/// *This run committed nothing, but its pipeline expects changes*: what the person may do with it, and why what they
/// cannot do now is unavailable.
pub fn flagged(
  ctx: &mut Ctx,
  (task, run): (&TaskContext, &RunView),
  action: &CompleteAction,
  gated: &Gated,
) -> Element {
  let anyway = ActionButton::new(
    "task-complete-anyway",
    ButtonKind::Secondary,
    ctx.t("decisions.complete.anyway"),
  )
  .icon(Icon::CircleCheck);
  let anyway = pressed(anyway, (task, run), action, Some(gated));
  let (back, back_reason) = return_button(ctx, task);
  let (again, again_reason) = run_again_button(ctx, task);
  let notice = Notice::new(
    "task-notice",
    NoticeTone::Warning,
    ctx.t("decisions.complete.flaggedTitle"),
  )
  .icon(Icon::TriangleAlert)
  .body(ctx.t_args(
    "decisions.complete.flaggedBody",
    [("branch", run.run.branch().as_str())],
  ))
  .stacked()
  .action(anyway)
  .action(back)
  .action(again);
  let reasons = [
    ("task-complete-return-reason", back_reason),
    ("task-complete-run-again-reason", again_reason),
  ]
  .into_iter()
  .filter_map(|(id, reason)| reason.map(|reason| Element::from(reason_line(id, &reason))));
  let person_only = gated.person_only.get().then(|| {
    Element::from(field_error(
      "task-complete-person-only",
      &ctx.t("decisions.complete.personOnly"),
    ))
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(notice)
    .with_children(reasons)
    .with_children(person_only)
    .with_children(refused(ctx, action))
    .into()
}

/// *Task completed*: who completed the run and when.
pub fn completed(ctx: &Ctx, run: &RunView, completion: &RunCompletion) -> Element {
  let key = match completion.by {
    CompletedBy::Person => "decisions.complete.completedByYou",
    CompletedBy::Automation => "decisions.complete.completedByAutomation",
    CompletedBy::Orchestrator => "decisions.complete.completedByOrchestrator",
  };
  let body = ctx.t_args(
    key,
    [
      ("time", display::clock(completion.completed_at)),
      ("run", run.run.number().get().to_string()),
    ],
  );
  Notice::new(
    "task-notice",
    NoticeTone::Success,
    ctx.t("decisions.complete.completedTitle"),
  )
  .icon(Icon::CircleCheck)
  .body(body)
  .into()
}

/// `button` completing `run` of `task` when pressed, anyway when `gated` (the person's leave for window input); under
/// way while a completion is. A completion by window input is automation's.
fn pressed(
  button: ActionButton,
  (task, run): (&TaskContext, &RunView),
  action: &CompleteAction,
  gated: Option<&Gated>,
) -> ActionButton {
  if action.state().get().is_pending() {
    return button.loading();
  }
  if task.detail.not_shared() {
    return button.disabled();
  }
  let (action, id, anyway) = (action.clone(), run.id(), gated.is_some());
  let gate = gated.map(|gated| (gated.gate.cloned(), gated.person_only.clone()));
  button.on_click(move || {
    if action.state().get_untracked().is_pending() {
      return;
    }
    let by = match &gate {
      Some((gate, person_only)) => {
        let Some(by) = answerer(gate.as_ref(), Guarded::Decision(DecisionKind::Stuck)) else {
          tracing::warn!("synthetic input completed a run anyway without consent; not taken");
          person_only.set(true);
          return;
        };
        person_only.set(false);
        by
      }
      None if is_synthetic_input() => Answerer::Automation,
      None => Answerer::Person,
    };
    action.run((id, CompletedBy::from(by), anyway));
  })
}

/// Why the last completion was refused.
fn refused(ctx: &Ctx, action: &CompleteAction) -> Option<Element> {
  let state = action.state().get();
  if state.is_pending() {
    return None;
  }
  let error = state.error?;
  Some(field_error("task-complete-refused", &message_text(ctx, &error)).into())
}

/// *Return to a step*: brings the return card beside the Summary into view, while the run's work can go back to one
/// of its agent steps.
fn return_button(ctx: &mut Ctx, task: &TaskContext) -> (ActionButton, Option<Arc<str>>) {
  let button = ActionButton::new(
    "task-complete-return",
    ButtonKind::Secondary,
    ctx.t("decisions.complete.returnToStep"),
  )
  .icon(Icon::CornerDownLeft);
  if super::super::returnable(task).is_none() {
    let reason = ctx.t("decisions.complete.returnUnavailable");
    return (button.unavailable(reason.clone()), Some(reason));
  }
  let project = task.project.open.project.id();
  let route = Route::Project(project, task.page(TaskView::of(TaskTab::Summary)));
  let pointed = Pointed {
    project,
    task: task.key.clone(),
    card: PointedCard::Return,
  };
  let navigation = Navigation::of(ctx);
  let button = button.on_click(move || navigation.open_to_card(&route, OpenIn::ActiveTab, pointed.clone()));
  (button, None)
}

/// *Run again*: opens Run again's confirmation under the header, while the task can run again after the run.
fn run_again_button(ctx: &mut Ctx, task: &TaskContext) -> (ActionButton, Option<Arc<str>>) {
  let services = Services::of(ctx);
  let feed = &task.project.feed;
  let again = (
    feed.revision.get() + feed.outside.get().tasks,
    feed.run_changes(feed.running.get()),
    feed.outside.get().limits,
  );
  let root = task.project.open.repo_root.clone();
  let check = run_again::check(ctx, (root, services.programs), &task.detail, again);
  let button = ActionButton::new(
    "task-complete-run-again",
    ButtonKind::Secondary,
    ctx.t("decisions.complete.runAgain"),
  )
  .icon(Icon::RotateCcw);
  let reason = match check {
    None => return (button.disabled(), None),
    Some(Err(error)) => message_text(ctx, &error),
    Some(Ok(check)) => match check.next {
      Err(block) => message_text(ctx, &block_message(block, &check.pipeline)),
      Ok(_) if !task.online => ctx.t("task.controls.offline"),
      Ok(_) => {
        let state = PageState::of(ctx);
        let (open, remove) = (state.run_again, state.run_again_remove);
        let button = button.on_click(move || {
          remove.set(false);
          open.set(Confirming(Some(IdempotencyKey::generate())));
        });
        return (button, None);
      }
    },
  };
  (button.unavailable(reason.clone()), Some(reason))
}
