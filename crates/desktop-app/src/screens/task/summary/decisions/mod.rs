//! The decisions a task's Summary waits for, in place of its usual sections (`Task page` `iwrwB`): the
//! plan review with its pinned decision bar (`ciGbZ`, [`plan`]), raising the limit a run stopped at
//! (`oTzJZ`, [`limit`]), the decision after a step used up its retries (`DVuXu`, [`retries`]), and Ready
//! for review with Merge into main and the merge's states (`F2LtFn`, `kxMAN`, [`ready`]). Each keeps the
//! Summary's frame: where the task stands, the run's stages ([`stages`]) in place of its step rows, the
//! decision, and *This run* beside them. A run stopped by the project's daily budget has nothing to
//! decide here: it goes on with Resume run in the header once the day has room. A run that ended may still
//! go back to one of its steps: that card stands above *Run details* ([`return_ended`]).

mod action;
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

pub(super) use self::return_ended::return_card;
use super::{
  column, layout,
  notice::{self, Said},
  runs::{self, Runs},
  this_run,
};
use crate::{
  navigation::Navigation,
  routes::{Route, TaskTab, TaskView},
  screens::task::{PageState, TaskContext},
  theme::DecisionText,
  ui::pressable::opening,
};

/// A divider's thickness.
const DIVIDER: f32 = 1.0;

/// The Summary of a task whose latest run waits for a decision: the plan review, raising the limit, the
/// decision after its retries, or Ready for review with its merge; `None` for the usual Summary.
pub fn decisions(ctx: &mut Ctx, props: &TaskContext) -> Option<Element> {
  let detail = &props.detail;
  if detail.latest_unreadable.is_some() {
    return None;
  }
  let run = detail.run.as_ref()?;
  let id = run.id();
  if let Some(review) = PlanReview::of(run) {
    // A new version is a new review: its note starts empty.
    let waiting = review.waiting();
    let key = format!("decision-plan-{id}-{}-{}", waiting.execution, waiting.version);
    return Some(ctx.mount_keyed::<plan::PlanDecision>(&key, props.clone()));
  }
  match run.run.status() {
    RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit { .. })) => {
      let key = format!("decision-plan-limit-{id}-{}", run.run.last_sequence().get());
      Some(ctx.mount_keyed::<plan_limit::PlanLimitDecision>(&key, props.clone()))
    }
    RunStatus::Paused(PauseReason::LimitReached(limit)) if *limit != RunLimit::DailyBudget => {
      let key = format!("decision-limit-{id}-{}", run.run.last_sequence().get());
      Some(ctx.mount_keyed::<limit::RaiseLimit>(&key, props.clone()))
    }
    RunStatus::WaitingForPerson(_) => {
      let key = format!("decision-retries-{id}-{}", run.run.last_sequence().get());
      Some(ctx.mount_keyed::<retries::RetryDecision>(&key, props.clone()))
    }
    // Work git finds merged without Orchester, or a run that committed nothing, waits for no merge: the notice
    // says which.
    RunStatus::Succeeded if props.detail.delivery == Delivery::Waiting => {
      Some(ctx.mount_keyed::<ready::ReadyForReview>(&format!("decision-ready-{id}"), props.clone()))
    }
    _ => None,
  }
}

/// Where the task stands, as the usual Summary says it, without its actions: the decision below is
/// where the person acts. A decision may word it closer to its own design before it is shown.
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

/// *This run* beside the decision, `lead` above it.
fn this_run_panel(ctx: &mut Ctx, props: &TaskContext, lead: Option<Element>) -> Option<Element> {
  let run = props.detail.run.as_ref()?;
  let lost = props.lost();
  Some(this_run::this_run(
    ctx,
    run,
    (&props.project.open.repo_root, lost),
    (lead, None),
  ))
}

/// *Runs of this task*, as the usual Summary ends.
fn runs_section(ctx: &mut Ctx, props: &TaskContext) -> Option<Element> {
  let removal = PageState::of(ctx).removal.get();
  runs::section(ctx, &Runs { task: props, removal })
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

/// The Summary's column of `parts` with `side` beside it; `bar` stays under the column while the column
/// scrolls (`Decision bar` `G5378f`).
fn arranged(ctx: &mut Ctx, parts: Vec<Element>, bar: Option<Element>, side: Option<Element>) -> Element {
  let scrolled = column(parts, PageState::of(ctx).summary_scroll);
  let Some(bar) = bar else {
    return layout(scrolled, side);
  };
  let pinned = Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(scrolled),
    )
    .child(bar);
  layout(pinned.into(), side)
}
