//! The end of a task's latest run in the Orchestrator's conversation (`docs/design/orchestrator-toolset.md`,
//! "Decisions" 5): in the action card's frame (`Action card` `nQIhx`), as an agent's wait ([`super::wait`]), its head
//! with how the run ended or stopped, the task and the run, and at its end how it stands and when it was noticed;
//! what happens with it (the Orchestrator takes it up by itself while *Let the Orchestrator answer agents* is on, else
//! it waits for the person); and *Open* the task's Summary. No Pencil frame draws it yet: it keeps to the wait card's
//! look.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{AgentWait, RunEnd},
  project::TaskKey,
  run::RunLimit,
  time::Timestamp,
};
use orchester_workspace::display;

use super::card::{self, State};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// The card of `wait`, the end `end` of its run, kept at `at`, which started an Orchestrator turn by itself when
/// `turned`; in project `project`; `id` names it.
pub(super) fn card(
  ctx: &mut Ctx,
  id: &str,
  (wait, end, at, turned): (&AgentWait, RunEnd, Option<Timestamp>, bool),
  project: ProjectId,
) -> Element {
  let run = wait.number.to_string();
  let step = wait.step.as_deref().unwrap_or_default();
  let (glyph, color) = look(end);
  let title = ctx.t_args(
    title(end, wait.step.is_some()),
    [("task", wait.task.as_str()), ("run", run.as_str()), ("step", step)],
  );
  let time = at.map(display::clock).unwrap_or_default();
  let state = State {
    glyph,
    color,
    words: ctx.t_args("orchestrator.end.noticed", [("time", time.as_str())]),
  };
  let effect = ctx.t(effect(wait, end, turned));
  card::frame(id, name(end))
    .describe("automatic", wait.automatic.to_string())
    .child(card::head(id, (glyph, &title), None, state))
    .child(
      Text::new(&effect)
        .id(format!("{id}-effect"))
        .variant(TypographyStyle::Body)
        .width(Dimension::Pct(100.0)),
    )
    .with_children(open_summary(ctx, id, project, &wait.task))
    .into()
}

/// The icon of `end` and the color of its state.
const fn look(end: RunEnd) -> (Icon, PaletteColor) {
  match end {
    RunEnd::Succeeded => (Icon::GitMerge, PaletteColor::Success),
    RunEnd::Failed => (Icon::CircleX, PaletteColor::Danger),
    RunEnd::Stopped => (Icon::Hand, PaletteColor::Warning),
    RunEnd::Limit { .. } => (Icon::Gauge, PaletteColor::Warning),
    RunEnd::Interrupted => (Icon::CirclePause, PaletteColor::Warning),
  }
}

/// The title of `end`, naming the step it stopped at when it is `named`.
const fn title(end: RunEnd, named: bool) -> &'static str {
  match (end, named) {
    (RunEnd::Succeeded, _) => "orchestrator.end.succeeded",
    (RunEnd::Failed, true) => "orchestrator.end.failedAt",
    (RunEnd::Failed, false) => "orchestrator.end.failed",
    (RunEnd::Stopped, true) => "orchestrator.end.stoppedAt",
    (RunEnd::Stopped, false) => "orchestrator.end.stopped",
    (RunEnd::Limit { limit }, _) => match limit {
      RunLimit::Tokens => "orchestrator.end.limitTokens",
      RunLimit::Time => "orchestrator.end.limitTime",
      RunLimit::DailyBudget => "orchestrator.end.limitDaily",
      RunLimit::GroupBudget => "orchestrator.end.limitGroup",
      RunLimit::StepBudget { .. } => "orchestrator.end.limitStep",
      RunLimit::PlanLimit { .. } => "orchestrator.end.limitPlan",
    },
    (RunEnd::Interrupted, _) => "orchestrator.end.interrupted",
  }
}

/// What happens with `end` of `wait`'s run: the Orchestrator takes it up by itself (in the turn it `turned`, or after
/// the turn under way), the Orchestrator's own decision on the run led to it so it waits for the person, or the
/// person's switch is off.
const fn effect(wait: &AgentWait, end: RunEnd, turned: bool) -> &'static str {
  match (end, wait.automatic, turned) {
    (RunEnd::Succeeded, true, true) => "orchestrator.end.effectReadyAnswering",
    (_, true, true) => "orchestrator.end.effectAnswering",
    (_, true, false) => "orchestrator.end.effectQueued",
    (_, false, _) if wait.own => "orchestrator.end.effectOwn",
    (_, false, _) => "orchestrator.end.effectWaits",
  }
}

/// *Open* the task's Summary, where the person decides what comes next.
fn open_summary(ctx: &mut Ctx, id: &str, project: ProjectId, task: &str) -> Option<Element> {
  let key: TaskKey = task.parse().ok()?;
  let navigation = Navigation::of(ctx);
  let label = ctx.t_args("orchestrator.end.open", [("task", task)]);
  let route = Route::Project(project, ProjectPage::Task(key, TaskView::of(TaskTab::Summary)));
  Some(
    Row::new()
      .child(
        ActionButton::new(format!("{id}-open"), ButtonKind::Ghost, label)
          .icon(Icon::ArrowUpRight)
          .on_click(move || navigation.go(route.clone())),
      )
      .into(),
  )
}

/// The name tools read of `end`.
const fn name(end: RunEnd) -> &'static str {
  match end {
    RunEnd::Succeeded => "run_succeeded",
    RunEnd::Failed => "run_failed",
    RunEnd::Stopped => "run_stopped",
    RunEnd::Limit { .. } => "run_limit",
    RunEnd::Interrupted => "run_interrupted",
  }
}
