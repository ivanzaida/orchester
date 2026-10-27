//! The decisions a task's Summary waits for, as a card at the top of its column (ORC-100, `01c` `SaKMC`, `Decision
//! cards` `CqVv3`): the plan review with its note and decision (`ciGbZ`, [`plan`]), raising the limit a run stopped at
//! (`oTzJZ`, [`limit`]), continuing a run its account's plan limit stopped ([`plan_limit`]), the decision after a step
//! used up its retries (`DVuXu`, [`retries`]), Ready for review with Merge into main and the merge's states (`F2LtFn`,
//! `kxMAN`, [`ready`]), a run its pipeline's Deliver step hands to the merge queue ([`delivery`]), and for a run that
//! committed nothing *Complete task*, or the warning that changes were expected ([`completion`], ORC-93). Each keeps
//! its logic and controls; the run's rail, its last event, its runs and facts stay the Summary's around it. A run
//! stopped by the project's daily budget has nothing to decide here: it goes on with Resume run in the header once the
//! day has room. A run that ended may still go back to one of its steps: that card stands at the top too
//! ([`return_ended`]).

mod action;
mod completion;
mod delivery;
mod limit;
mod plan;
mod plan_limit;
mod pointing;
mod ready;
mod retries;
mod return_ended;
mod stages;

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::run::{PauseReason, RunLimit, RunStatus};
use orchester_workspace::tasks::{delivery::Delivery, detail::StepView, plan::PlanReview};

pub use self::return_ended::returnable;
pub(super) use self::{delivery::details::rows as delivery_rows, return_ended::return_card};
use super::notice::{self, Said};
use crate::{
  navigation::Navigation,
  routes::{Route, TaskTab, TaskView},
  screens::task::{PageState, TaskContext},
  theme::{DecisionText, TaskSpacing},
  ui::pressable::opening,
};

/// A divider's thickness.
const DIVIDER: f32 = 1.0;

/// The decision the task's latest run waits for on the Summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waiting {
  /// The plan review.
  Plan,
  /// A run its pipeline's Deliver step hands to the merge queue.
  Delivery,
  /// The CLI's plan limit stopped a step.
  PlanLimit,
  /// Raising the limit the run stopped at.
  Limit,
  /// How the run goes on after a step's retries.
  Retries,
  /// Ready for review, with its merge.
  Ready,
  /// Completing a run that committed nothing (ORC-93).
  Completion,
}

/// The decision the task's latest run waits for on the Summary; `None` when it waits for none there.
pub fn waiting(props: &TaskContext) -> Option<Waiting> {
  let detail = &props.detail;
  if detail.latest_unreadable.is_some() {
    return None;
  }
  let run = detail.run.as_ref()?;
  if PlanReview::of(run).is_some() {
    return Some(Waiting::Plan);
  }
  // A run its pipeline's Deliver step hands to the merge queue is not merged from here.
  if delivery::shown(props) {
    return Some(Waiting::Delivery);
  }
  match run.run.status() {
    RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit { .. })) => Some(Waiting::PlanLimit),
    RunStatus::Paused(PauseReason::LimitReached(limit)) if *limit != RunLimit::DailyBudget => Some(Waiting::Limit),
    RunStatus::WaitingForPerson(_) => Some(Waiting::Retries),
    // Work git finds merged without Orchester waits for no merge: the run's line says so.
    RunStatus::Succeeded if props.detail.delivery == Delivery::Waiting => Some(Waiting::Ready),
    // A run that committed nothing is completed rather than merged, or flagged when changes were expected (ORC-93).
    RunStatus::Succeeded if completion::shown(props) => Some(Waiting::Completion),
    _ => None,
  }
}

/// The card of the decision the task's latest run waits for, at the top of the Summary's column: the plan review,
/// raising the limit, the decision after its retries, or Ready for review with its merge; `None` when it waits for
/// none.
pub fn decisions(ctx: &mut Ctx, props: &TaskContext) -> Option<Element> {
  let run = props.detail.run.as_ref()?;
  let id = run.id();
  let sequence = run.run.last_sequence().get();
  Some(match waiting(props)? {
    Waiting::Plan => {
      let review = PlanReview::of(run)?;
      // A new version is a new review: its note starts empty.
      let waiting = review.waiting();
      let key = format!("decision-plan-{id}-{}-{}", waiting.execution, waiting.version);
      ctx.mount_keyed::<plan::PlanDecision>(&key, props.clone())
    }
    Waiting::Delivery => {
      ctx.mount_keyed::<delivery::DeliveryDecision>(&format!("decision-delivery-{id}"), props.clone())
    }
    Waiting::PlanLimit => {
      let key = format!("decision-plan-limit-{id}-{sequence}");
      ctx.mount_keyed::<plan_limit::PlanLimitDecision>(&key, props.clone())
    }
    Waiting::Limit => ctx.mount_keyed::<limit::RaiseLimit>(&format!("decision-limit-{id}-{sequence}"), props.clone()),
    Waiting::Retries => {
      ctx.mount_keyed::<retries::RetryDecision>(&format!("decision-retries-{id}-{sequence}"), props.clone())
    }
    Waiting::Ready => ctx.mount_keyed::<ready::ReadyForReview>(&format!("decision-ready-{id}"), props.clone()),
    Waiting::Completion => {
      ctx.mount_keyed::<completion::CompleteRun>(&format!("decision-complete-{id}"), props.clone())
    }
  })
}

/// Where the task stands, as the Summary's run says it, without its actions: the decision is where the person acts.
fn task_notice(ctx: &Ctx, props: &TaskContext) -> Said {
  let detail = &props.detail;
  let lost = props.lost();
  let runner = props
    .project
    .feed
    .status
    .get()
    .map(|status| status.state_for(&props.project.open.repo_root));
  let approval = props.waiting_approval();
  notice::said(ctx, detail, runner.as_ref().filter(|_| lost), approval.as_ref())
}

/// Where the task stands, for the run's last event: as the decision it waits for words it, closer to its own design
/// (the limit's checkpoint, how often the work went back after its retries, the account a plan limit stopped).
pub fn said(ctx: &mut Ctx, props: &TaskContext) -> Said {
  let said = task_notice(ctx, props);
  let Some(run) = props.detail.run.as_ref() else {
    return said;
  };
  match (waiting(props), run.run.status()) {
    (Some(Waiting::Limit), RunStatus::Paused(PauseReason::LimitReached(limit))) => {
      limit::said(ctx, props, (run, *limit), said)
    }
    (Some(Waiting::Retries), RunStatus::WaitingForPerson(stuck)) => retries::said(ctx, run, stuck, said),
    // The plan limit's card reads the stop, and words it.
    (Some(Waiting::PlanLimit), _) => match PageState::of(ctx).event.get() {
      Some(words) => words.apply(run, said),
      None => said,
    },
    _ => said,
  }
}

/// "03 Implement": a step's number in the pipeline and its name.
fn step_label(ctx: &Ctx, step: &StepView) -> Arc<str> {
  ctx.t_args(
    "decisions.stepLabel",
    [
      ("number", format!("{:02}", step.number)),
      ("step", step.name.to_string()),
    ],
  )
}

/// "Changes ↗": a link to another tab of the task, worded by `label` (`bUBJC`, `hFIV4`).
fn tab_link(ctx: &mut Ctx, props: &TaskContext, (id, label): (&str, &str), tab: TaskTab) -> Element {
  let navigation = Navigation::of(ctx);
  let route = Route::Project(props.project.open.project.id(), props.page(TaskView::of(tab)));
  opening(navigation.to(route))
    .id(id.to_owned())
    .child(Text::new(&ctx.t(label)).variant(DecisionText::Link.role()).nowrap())
    .into()
}

/// A 1 `border` line across a card (`B5pLJk`, `Ybgq7`).
fn divider() -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(DIVIDER))
    .background(PaletteColor::Border)
    .into()
}

/// A decision's `parts` at the top of the Summary's column: its card first, then what it rests on.
fn arranged(parts: Vec<Element>) -> Element {
  Column::new()
    .id("task-decision")
    .width(Dimension::Pct(100.0))
    .spacing(TaskSpacing::Block.role())
    .with_children(parts)
    .into()
}
