//! *This run* beside the Summary (`This run` `W73m1n` in `lUXff`): what the run pinned and where it
//! works, its limits and usage, its active agent time going on while an agent works, and what the
//! current step's agent may do. A task without a run shows *This task* instead: the pipeline and limits
//! its first run would use. A limit that is not set reads *No limit*. The run's approvals section and
//! its permissions popover come later (agreed deviation D4).

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor},
  },
  components::{Column, ScrollVertical, Text},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::RepositoryAccess,
  run::{ActiveClock, RunStatus, StepStatus},
  task::TaskLimits,
  tokens::{TokenBudget, compact_tokens},
};
use orchester_workspace::{
  budget::{unbounded, whole_minutes},
  display,
  tasks::{
    changes::run_worktree,
    data::PipelineChoice,
    detail::{RunView, StepView, TaskDetail},
  },
};

use super::live::Ticker;
use crate::{
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, TaskSpacing, TaskText},
  ui::{
    detail_row::{DetailValue, detail_row, detail_row_of},
    message::message_text,
  },
};

/// The panel's width (`W73m1n`).
const PANEL_WIDTH: f32 = 340.0;
/// Characters of the worktree's path shown before its middle gives way to an ellipsis.
const PATH_CHARS: usize = 34;

/// The panel around `rows`, scrolling when they are taller than the window.
pub(super) fn panel(id: &str, title: &str, rows: Vec<Element>) -> Element {
  let content = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(TaskSpacing::Panel.role())
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Column::new()
        .padding(Padding::new().bottom(ExtraSpacing::Snug.role()))
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(TaskText::SectionTitle.role()),
        ),
    )
    .with_children(rows);
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Px(PANEL_WIDTH))
    .height(Dimension::Pct(100.0))
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(ScrollVertical::new(content).width(Dimension::Pct(100.0)).flex(1.0))
    .into()
}

/// *This run* of `run`, its active agent time counting on while the run has a runner (not `lost`).
pub fn this_run(ctx: &mut Ctx, run: &RunView, root: &Path, lost: bool) -> Element {
  let version = run.version.to_string();
  let pipeline = ctx.t_args(
    "summary.thisRun.pipelineValue",
    [("pipeline", &*run.pipeline), ("version", version.as_str())],
  );
  let mut rows: Vec<Element> = vec![
    row(ctx, "summary.thisRun.pipeline", &pipeline, DetailValue::Text),
    row(
      ctx,
      "summary.thisRun.branch",
      run.run.branch().as_str(),
      DetailValue::Mono,
    ),
    row(ctx, "summary.thisRun.base", &base_value(ctx, run), DetailValue::Mono),
    checkpoint_row(ctx, run),
    row(ctx, "summary.thisRun.model", &model(ctx, run), DetailValue::Text),
    budget_row(ctx, run),
    cached_row(ctx, run),
  ];
  rows.push(row(
    ctx,
    "summary.thisRun.timeLimit",
    &time_limit(ctx, run.run.max_duration()),
    DetailValue::Text,
  ));
  let active = run.active_clock(&run.invocations, Services::of(ctx).clock.now(), lost);
  rows.push(ctx.mount_keyed::<ActiveTime>("task-active-time", ActiveProps { active }));
  rows.extend(permissions(ctx, run));
  rows.push(worktree(ctx, run, root));
  panel("task-this-run", &ctx.t("summary.thisRun.title"), rows)
}

/// "main · 4d2b8e0": the branch the run started from at the revision it started at.
pub(super) fn base_value(ctx: &Ctx, run: &RunView) -> Arc<str> {
  let revision = display::short_revision(run.run.base().as_str());
  match run.run.base_branch() {
    Some(branch) => ctx.t_args(
      "summary.thisRun.baseValue",
      [("branch", branch.as_str()), ("revision", revision)],
    ),
    None => Arc::from(revision),
  }
}

/// *Last checkpoint*: its revision and time, or that the run has none yet.
pub(super) fn checkpoint_row(ctx: &Ctx, run: &RunView) -> Element {
  let Some(checkpoint) = run.run.last_checkpoint() else {
    return row(
      ctx,
      "summary.thisRun.checkpoint",
      &ctx.t("summary.thisRun.noCheckpoint"),
      DetailValue::Text,
    );
  };
  let value = ctx.t_args(
    "summary.thisRun.checkpointValue",
    [
      (
        "revision",
        display::short_revision(checkpoint.revision().as_str()).to_owned(),
      ),
      ("time", display::clock(checkpoint.created_at())),
    ],
  );
  row(ctx, "summary.thisRun.checkpoint", &value, DetailValue::Mono)
}

/// *Cached input*: what the run read from the providers' caches, which its budget does not count.
pub(super) fn cached_row(ctx: &Ctx, run: &RunView) -> Element {
  let cached = ctx.t_args(
    "summary.thisRun.cachedValue",
    [("count", compact_tokens(run.usage().cached_input))],
  );
  row(ctx, "summary.thisRun.cached", &cached, DetailValue::Text)
}

/// *Budget*, in `warning` once the run used it up (`MI115` in `oTzJZ`).
fn budget_row(ctx: &Ctx, run: &RunView) -> Element {
  let color = if run.run.token_limit_reached() {
    PaletteColor::Warning
  } else {
    PaletteColor::TextPrimary
  };
  let value = Text::new(&budget(ctx, run))
    .id("task-this-run-budget")
    .variant(ExtraTypography::Prose.role())
    .color(color)
    .flex(1.0);
  detail_row_of("task-this-run-budget", &ctx.t("summary.thisRun.budget"), value).into()
}

/// The row of `key`, a key of `summary.thisRun`; its value's element id ends in the key's last part.
pub(super) fn row(ctx: &Ctx, key: &str, value: &str, kind: DetailValue) -> Element {
  let name = key.rsplit('.').next().unwrap_or(key);
  detail_row(&format!("task-this-run-{name}"), &ctx.t(key), value, kind).into()
}

/// The step whose agent *This run* describes: the current step, or while that is a tool step, the
/// last agent step that ran.
pub(super) fn agent_step(run: &RunView) -> Option<&StepView> {
  let ran = |step: &StepView| {
    matches!(
      step.status,
      Some(StepStatus::Running | StepStatus::Waiting(_) | StepStatus::Passed | StepStatus::Failed)
    )
  };
  let current = run.current_step()?;
  if !current.tool_step && ran(current) {
    return Some(current);
  }
  run
    .steps
    .iter()
    .rev()
    .find(|step| !step.tool_step && ran(step))
    .or(Some(current))
}

/// "sonnet-4.5 (Builder)" of the agent step; a tool step's command runs on no model.
fn model(ctx: &Ctx, run: &RunView) -> Arc<str> {
  let Some(step) = agent_step(run) else {
    return ctx.t("summary.thisRun.none");
  };
  if step.tool_step {
    return ctx.t("summary.thisRun.commandTool");
  }
  match (step.model.as_deref(), step.agent.as_deref()) {
    (Some(model), Some(agent)) => ctx.t_args("summary.thisRun.modelValue", [("model", model), ("agent", agent)]),
    (Some(model), None) => Arc::from(model),
    (None, _) => ctx.t("summary.thisRun.none"),
  }
}

/// "412k / 1.4M tokens", that the limit is reached, or "No limit · 412k tokens used".
fn budget(ctx: &Ctx, run: &RunView) -> Arc<str> {
  let used = compact_tokens(run.run.used().get());
  if is_unlimited(run.run.budget()) {
    return ctx.t_args("summary.thisRun.budgetNoLimit", [("used", used)]);
  }
  let key = if run.run.token_limit_reached() {
    "summary.thisRun.budgetReached"
  } else {
    "summary.thisRun.budgetValue"
  };
  ctx.t_args(
    key,
    [("used", used), ("budget", compact_tokens(run.run.budget().get()))],
  )
}

/// A budget without a limit is stored as the largest one (`budget::unbounded`).
pub(super) fn is_unlimited(budget: TokenBudget) -> bool {
  budget == unbounded()
}

fn time_limit(ctx: &Ctx, limit: std::time::Duration) -> Arc<str> {
  ctx.t_args(
    "summary.thisRun.minutes",
    [("minutes", whole_minutes(limit).to_string())],
  )
}

/// What the current step's agent may do: "Running with · Builder" over "Edit files · 3 commands ·
/// Network off · MCP asks"; nothing for a tool step, or once its profile version is gone.
pub(super) fn permissions(ctx: &Ctx, run: &RunView) -> Option<Element> {
  let step = agent_step(run)?;
  let permissions = step.permissions.as_ref()?;
  let agent = step.agent.as_deref().unwrap_or_default();
  // Only a run at work runs with its agent; one that stopped, paused or ended last ran with it.
  let lead_key = if *run.run.status() == RunStatus::Running {
    "summary.thisRun.runningWith"
  } else {
    "summary.thisRun.lastRanWith"
  };
  let summary = permission_words(ctx, &permissions.permissions, permissions.tool_approvals_required);
  let value = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(&ctx.t_args(lead_key, [("agent", agent)]))
        .id("task-this-run-permissions-lead")
        .variant(ExtraTypography::Note.role()),
    )
    .child(
      Text::new(&summary)
        .id("task-this-run-permissions")
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::TextPrimary),
    );
  Some(
    detail_row_of(
      "task-this-run-permissions-row",
      &ctx.t("summary.thisRun.permissions"),
      value,
    )
    .into(),
  )
}

fn permission_words(ctx: &Ctx, permissions: &AgentPermissions, mcp_asks: bool) -> Arc<str> {
  let files = match permissions.repository() {
    RepositoryAccess::ReadOnly => ctx.t("summary.thisRun.readOnly"),
    RepositoryAccess::Write => ctx.t("summary.thisRun.editFiles"),
  };
  let commands = match permissions.commands() {
    CommandPermission::AskBefore => ctx.t("summary.thisRun.commandsAsk"),
    CommandPermission::Allowed(listed) => ctx.t_args(
      "summary.thisRun.commandsListed",
      [("count", listed.entries().len().to_string())],
    ),
    CommandPermission::AnyInWorktree => ctx.t("summary.thisRun.commandsAny"),
  };
  let network = match permissions.network() {
    NetworkAccess::Off => ctx.t("summary.thisRun.networkOff"),
    NetworkAccess::On => ctx.t("summary.thisRun.networkOn"),
  };
  let key = if mcp_asks {
    "summary.thisRun.permissionsMcpAsks"
  } else {
    "summary.thisRun.permissionsValue"
  };
  ctx.t_args(key, [("files", files), ("commands", commands), ("network", network)])
}

/// Where the run works: its worktree's folder, or when it was removed.
fn worktree(ctx: &Ctx, run: &RunView, root: &Path) -> Element {
  match run.worktree_removed {
    Some(removed) => {
      let value = ctx.t_args("summary.thisRun.worktreeRemoved", [("time", display::clock(removed))]);
      row(ctx, "summary.thisRun.worktree", &value, DetailValue::Text)
    }
    None => {
      let path = display::shorten_path(&run_worktree(root, run.id()), PATH_CHARS);
      row(ctx, "summary.thisRun.worktree", &path, DetailValue::Mono)
    }
  }
}

/// *This task* of a task without a run: the pipeline and limits its first run would use.
pub fn this_task(ctx: &mut Ctx, detail: &TaskDetail, pipelines: &[PipelineChoice]) -> Element {
  let details = detail.task.details();
  let pipeline = details
    .pipeline
    .and_then(|chosen| pipelines.iter().find(|pipeline| pipeline.id == chosen))
    .map_or_else(
      || ctx.t("summary.thisRun.notChosen"),
      |pipeline| {
        ctx.t_args(
          "summary.thisRun.pipelineValue",
          [
            ("pipeline", &*pipeline.name),
            ("version", pipeline.version.to_string().as_str()),
          ],
        )
      },
    );
  let mut rows = vec![row(ctx, "summary.thisRun.pipeline", &pipeline, DetailValue::Text)];
  match details.limits {
    Some(limits) => rows.extend(limit_rows(ctx, limits)),
    None => rows.push(row(
      ctx,
      "summary.thisRun.budget",
      &ctx.t("summary.thisRun.notChosen"),
      DetailValue::Text,
    )),
  }
  panel("task-this-task", &ctx.t("summary.thisTask.title"), rows)
}

fn limit_rows(ctx: &Ctx, limits: TaskLimits) -> [Element; 2] {
  let budget = if is_unlimited(limits.budget()) {
    ctx.t("summary.thisRun.noLimit")
  } else {
    ctx.t_args(
      "summary.thisRun.tokens",
      [("budget", compact_tokens(limits.budget().get()))],
    )
  };
  [
    row(ctx, "summary.thisRun.budget", &budget, DetailValue::Text),
    row(
      ctx,
      "summary.thisRun.timeLimit",
      &time_limit(ctx, limits.max_duration()),
      DetailValue::Text,
    ),
  ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ActiveProps {
  active: ActiveClock,
}

impl lurq::app::component::DevtoolsInspectable for ActiveProps {}

/// *Active agent time*, counting on while an agent works.
struct ActiveTime {
  ticker: Ticker,
}

impl Component for ActiveTime {
  type Props = ActiveProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      ticker: Ticker::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let active = ctx.props::<ActiveProps>().active;
    let now = self.ticker.now(ctx, active.counting());
    let time = message_text(ctx, &display::duration(active.at(now)));
    row(ctx, "summary.thisRun.active", &time, DetailValue::Text)
  }
}
