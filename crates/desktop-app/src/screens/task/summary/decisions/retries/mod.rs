//! Waits after retries (`Task · Summary · waits after retries` `DVuXu`): a step kept failing after its
//! failure route was taken as often as it allows, or broke again after its restart, and the run waits for
//! the person. Under where the task stands and the run's stages, the evidence ([`evidence`]): the latest
//! attempt's steps, what the failing step reported, and that the changes and messages are kept. Above *This
//! run*, the decision ([`decide`]): *Allow 1 retry & continue*, returning the work to an agent step with a
//! note its agent reads, and *Stop and fail the run*; the runner carries out each.

mod decide;
mod evidence;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::{
  resource::StepKey,
  run::{RunStatus, StuckDecision},
};

use self::decide::DecideCard;
use super::{TaskContext, action::decision_action, arranged, stages::stages, task_notice, this_run_panel};
use crate::services::Services;

/// Which decision was asked for, for the button that shows it under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deciding {
  Retry,
  Return,
  Stop,
}

impl lurq::app::component::DevtoolsInspectable for Deciding {}

impl Deciding {
  fn of(decision: &StuckDecision) -> Self {
    match decision {
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

pub struct RetryDecision {
  target: Signal<Target>,
  note: Signal<String>,
  asked: Signal<Option<Deciding>>,
}

impl Component for RetryDecision {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      target: ctx.signal(Target(None)),
      note: ctx.signal(String::new()),
      asked: ctx.signal(None),
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
    // The choice starts at the step one more retry resumes, else at the first the work can go back to.
    if self.target.get_untracked().0.is_none() {
      let targets = run.run.return_targets();
      let start = run
        .run
        .retry_from()
        .filter(|from| targets.contains(from))
        .or_else(|| targets.first())
        .cloned();
      self.target.set(Target(start));
    }
    let now = Services::of(ctx).clock.now();
    let decide = DecideCard {
      run,
      stuck,
      target: &self.target,
      note: &self.note,
      asked: &self.asked,
      action: &action,
    }
    .render(ctx);
    let parts: Vec<Element> = std::iter::once(task_notice(ctx, &props).into())
      .chain(std::iter::once(stages(run, Some(stuck.step().as_str()), now)))
      .chain(evidence::sections(ctx, &props, run, stuck.step(), now))
      .collect();
    let side = this_run_panel(ctx, &props, Some(decide));
    arranged(parts, None, side)
  }
}
