//! Waits after retries (`Task · Summary · waits after retries` `DVuXu`): a step kept failing after its
//! failure route was taken as often as it allows, or broke again after its restart, and the run waits for
//! the person. Under where the task stands and the run's stages, the evidence ([`evidence`]): the latest
//! attempt's steps, what the failing step reported, and that the changes and messages are kept. Above *This
//! run*, the decision ([`decide`]): *Allow 1 retry & continue*, returning the work to an agent step with a
//! note its agent reads, and *Stop and fail the run*; the runner carries out each.
//!
//! A step that reported unverified, or was not run again on an unchanged revision, waits the same way, with
//! what it could not check above the evidence ([`unverified`]) and *Accept … as it is* and *Run … again* in
//! place of the retry. The design has no frame of its own for it yet; it reuses the retry decision's parts.

mod decide;
mod evidence;
mod unverified;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::{
  resource::StepKey,
  run::{RunStatus, StepFailure, Stuck, StuckDecision},
};
use orchester_workspace::tasks::detail::RunView;

use self::decide::DecideCard;
use super::{
  TaskContext, action::decision_action, arranged, pointing::Pointing, stages::stages, step_label, task_notice,
  this_run_panel,
};
use crate::{mcp::input_gate::InputGate, navigation::pointed::PointedCard, services::Services, ui::notice::Notice};

/// Which decision was asked for, for the button that shows it under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deciding {
  Accept,
  Retry,
  Return,
  Stop,
}

impl lurq::app::component::DevtoolsInspectable for Deciding {}

impl Deciding {
  fn of(decision: &StuckDecision) -> Self {
    match decision {
      StuckDecision::Accept { .. } => Self::Accept,
      StuckDecision::Retry => Self::Retry,
      StuckDecision::ReturnTo { .. } => Self::Return,
      StuckDecision::Stop => Self::Stop,
    }
  }
}

/// The step the work goes back to; `None` before the choice is seeded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target(Option<StepKey>);

impl lurq::app::component::DevtoolsInspectable for Target {}

/// Where the task stands; after used-up retries, how often and where the step sent the work back
/// (`Notice · Review still fails after 2 retries` in `DVuXu`).
fn notice(ctx: &Ctx, props: &TaskContext, run: &RunView, stuck: &Stuck) -> Notice {
  let mut said = task_notice(ctx, props);
  if stuck.can_accept() {
    return unverified_notice(ctx, run, stuck, said);
  }
  let Stuck::RetriesExhausted(step) = stuck else {
    return said.notice();
  };
  let view = |key: &StepKey| run.steps.iter().find(|view| view.key == *key);
  let (Some(failed), Some(target)) = (view(step), run.run.retry_from().and_then(view)) else {
    return said.notice();
  };
  let count = run.run.automatic_retries_taken(step);
  let args = [
    ("step", failed.name.to_string()),
    ("label", step_label(ctx, failed).to_string()),
    ("target", target.name.to_string()),
    ("count", count.to_string()),
  ];
  let (title, body) = if count == 1 {
    ("decisions.retries.noticeTitle.one", "decisions.retries.noticeBody.one")
  } else {
    (
      "decisions.retries.noticeTitle.other",
      "decisions.retries.noticeBody.other",
    )
  };
  said.title = ctx.t_args(title, args.clone());
  said.body = ctx.t_args(body, args);
  said.notice()
}

/// "Unverified: Verify could not check 2 things", or "Verify was not run again", with what that means.
fn unverified_notice(ctx: &Ctx, run: &RunView, stuck: &Stuck, mut said: super::Said) -> Notice {
  let Some(step) = run.steps.iter().find(|view| view.key == *stuck.step()) else {
    return said.notice();
  };
  let count = run.unverified().len();
  let args = [
    ("step", step.name.to_string()),
    ("label", step_label(ctx, step).to_string()),
    ("count", count.to_string()),
  ];
  let (title, body) = match (stuck, count) {
    (Stuck::Unverified(_), 1) => (
      "decisions.retries.unverifiedNoticeTitle.one",
      "decisions.retries.unverifiedNoticeBody",
    ),
    (Stuck::Unverified(_), _) => (
      "decisions.retries.unverifiedNoticeTitle.other",
      "decisions.retries.unverifiedNoticeBody",
    ),
    _ => (
      "decisions.retries.unchangedNoticeTitle",
      match run.unchanged_after() {
        Some(StepFailure::Unverified) => "decisions.retries.unchangedNoticeBody.unverified",
        Some(_) => "decisions.retries.unchangedNoticeBody.failed",
        None => "decisions.retries.unchangedNoticeBody.ended",
      },
    ),
  };
  said.title = ctx.t_args(title, args.clone());
  said.body = ctx.t_args(body, args);
  said.notice()
}

pub struct RetryDecision {
  target: Signal<Target>,
  note: Signal<String>,
  asked: Signal<Option<Deciding>>,
  /// Window input pressed *Accept … as it is*, which is a person's decision ([`Guarded::StepAccept`](
  /// crate::mcp::input_gate::Guarded::StepAccept)), without their leave.
  person_only: Signal<bool>,
  gate: Option<InputGate>,
  pointing: Pointing,
}

impl Component for RetryDecision {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      target: ctx.signal(Target(None)),
      note: ctx.signal(String::new()),
      asked: ctx.signal(None),
      person_only: ctx.signal(false),
      gate: ctx.use_context::<InputGate>(),
      pointing: Pointing::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TaskContext>().clone();
    let action = decision_action::<Deciding>(ctx, props.project.open.clone());
    let Some(run) = props.detail.run.as_ref() else {
      return Column::new().into();
    };
    let RunStatus::WaitingForPerson(stuck) = run.run.status() else {
      return Column::new().into();
    };
    // The choice starts at the run's default return target ([`orchester_core::run::Run::return_default`]).
    if self.target.get_untracked().0.is_none() {
      self.target.set(Target(run.run.return_default()));
    }
    self.pointing.arrived(ctx, &props, PointedCard::Retries);
    let now = Services::of(ctx).clock.now();
    let decide = DecideCard {
      run,
      stuck,
      target: &self.target,
      note: &self.note,
      asked: &self.asked,
      person_only: &self.person_only,
      action: &action,
      gate: self.gate.as_ref(),
      offline: (!props.online).then(|| ctx.t("task.controls.offline")),
      pointing: &self.pointing,
    }
    .render(ctx);
    let parts: Vec<Element> = std::iter::once(notice(ctx, &props, run, stuck).into())
      .chain(std::iter::once(stages(run, Some(stuck.step().as_str()), now)))
      .chain(unverified::section(ctx, run, stuck.step()))
      .chain(evidence::sections(ctx, &props, run, stuck.step(), now))
      .collect();
    let side = this_run_panel(ctx, &props, Some(decide));
    arranged(ctx, parts, None, side)
  }
}
