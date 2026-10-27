//! Waits after retries (`Decision · Stopped after 3 retries` `Q9sqJY` in `Decision cards` `CqVv3`): a step kept
//! failing after its failure route was taken as often as it allows, or broke again after its restart, and the run
//! waits for the person. At the top of the Summary the decision ([`decide`]): *Allow 1 retry & continue*, returning
//! the work to an agent step with a note its agent reads, and *Stop and fail the run*; the runner carries out each.
//! Under it the evidence ([`evidence`]): what the failing step reported, and that the changes and messages are kept.
//! The run's last event says how often and where the step sent the work back ([`said`]).
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
use super::{Said, TaskContext, action::decision_action, arranged, pointing::Pointing, step_label};
use crate::{mcp::input_gate::InputGate, navigation::pointed::PointedCard};

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

/// Where the task stands, from `said`; after used-up retries, how often and where the step sent the work back
/// (`Notice · Review still fails after 2 retries` in `DVuXu`); after a follow-up of a step without a failure route
/// failed (the run had been ready for review), that it did and that nothing went back.
pub fn said(ctx: &Ctx, run: &RunView, stuck: &Stuck, mut said: Said) -> Said {
  if stuck.can_accept() {
    return unverified_said(ctx, run, stuck, said);
  }
  let Stuck::RetriesExhausted(step) = stuck else {
    return said;
  };
  let view = |key: &StepKey| run.steps.iter().find(|view| view.key == *key);
  let (Some(failed), Some(target)) = (view(step), run.run.retry_from().and_then(view)) else {
    return said;
  };
  let count = run.run.automatic_retries_taken(step);
  let args = [
    ("step", failed.name.to_string()),
    ("label", step_label(ctx, failed).to_string()),
    ("target", target.name.to_string()),
    ("count", count.to_string()),
  ];
  let (title, body) = if run.run.waits_without_route() {
    (
      "decisions.retries.noRouteNoticeTitle",
      "decisions.retries.noRouteNoticeBody",
    )
  } else if count == 1 {
    ("decisions.retries.noticeTitle.one", "decisions.retries.noticeBody.one")
  } else {
    (
      "decisions.retries.noticeTitle.other",
      "decisions.retries.noticeBody.other",
    )
  };
  said.title = ctx.t_args(title, args.clone());
  said.body = ctx.t_args(body, args);
  said
}

/// "Unverified: Verify could not check 2 things", or "Verify was not run again", with what that means.
fn unverified_said(ctx: &Ctx, run: &RunView, stuck: &Stuck, mut said: Said) -> Said {
  let Some(step) = run.steps.iter().find(|view| view.key == *stuck.step()) else {
    return said;
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
  said
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
    let parts: Vec<Element> = std::iter::once(decide)
      .chain(unverified::section(ctx, run, stuck.step()))
      .chain(evidence::sections(ctx, &props, run, stuck.step()))
      .collect();
    arranged(parts)
  }
}
