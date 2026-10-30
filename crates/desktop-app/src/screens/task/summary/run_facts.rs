//! The run's facts under the task's properties (`Run group` `ZDVJZ` in `01b · Summary · full page`): after a hairline
//! and 16 over them, *Run 8* in 12/600 with *this run*, or *ended*, in 11.5 `text_muted`; then a row each, 6 over and
//! under it: Base, Checkpoint, Tokens of the run's budget (in `warning` once it is used up), Active time of its limit
//! (counting on while an agent works), Model and Worktree (Geist Mono, its middle cut, the whole path in its tooltip; a
//! press copies it). One disclosure row (`More run details` `GBA8T`), closed until opened, holds the rest in the same
//! rows: the account its latest agent started on, the cached input its budget does not count, how long it took and
//! its agents once it ended, what its current agent may do, and how the merge queue delivers it.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::RepositoryAccess,
  run::{ActiveClock, PauseReason, RunLimit, RunStatus, StepStatus},
  tokens::compact_tokens,
};
use orchester_workspace::{
  budget::whole_minutes,
  display,
  tasks::{
    changes::run_worktree,
    detail::{RunView, StepView},
  },
};

use super::{
  decisions::delivery_rows,
  live::Ticker,
  properties::{Look, row, value},
};
use crate::{
  screens::task::TaskContext,
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    clipboard::Clipboard,
    icon::{Icon, icon},
    message::message_text,
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// Characters of the worktree's path before its middle gives way to an ellipsis: what Geist Mono 12 fits in the value
/// column's 144 (`BAYWM`).
const PATH_CHARS: usize = 20;
const MILLIS_PER_MINUTE: i64 = 60_000;

/// A fact's row, 6 over and under it.
pub(super) fn fact(id: &str, key: &str, value: impl Into<Element>) -> Element {
  row(id, key, value, ExtraSpacing::Snug.role())
}

/// The group of `run`'s facts, the rest open while `more` holds.
pub fn group(ctx: &mut Ctx, task: &TaskContext, run: &RunView, more: &Signal<bool>) -> Element {
  let ended = run.run.status().is_terminal();
  let hint = if ended {
    ctx.t("summary.facts.ended")
  } else {
    ctx.t("summary.facts.thisRun")
  };
  let head = Row::new()
    .id("task-facts-head")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().bottom(SpacingSize::Xs))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t_args("summary.facts.run", [("run", run.run.number().get().to_string())]))
        .id("task-facts-title")
        .variant(ExtraTypography::InspectorSection.role())
        .nowrap(),
    )
    .child(
      Text::new(&hint)
        .id("task-facts-hint")
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    );
  let lost = task.lost();
  let active = run.active_clock(&run.invocations, Services::of(ctx).clock.now(), lost);
  let max = run.run.max_duration().map(whole_minutes);
  let at_limit = matches!(
    run.run.status(),
    RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit { .. }))
  );
  let mut rows = vec![
    fact(
      "task-fact-base",
      &ctx.t("summary.facts.base"),
      value("task-fact-base", &base_value(ctx, run), Look::Mono),
    ),
    checkpoint(ctx, run),
    tokens(ctx, run),
    ctx.mount_keyed::<ActiveTime>("task-fact-active", ActiveProps { active, max }),
    fact(
      "task-fact-model",
      &ctx.t("summary.facts.model"),
      value("task-fact-model", &model(ctx, run), Look::Secondary),
    ),
    worktree(ctx, run, &task.project.open.repo_root),
  ];
  // A run stopped at its account's plan limit is about that account: it stays in sight.
  if at_limit {
    rows.extend(account(ctx, run, true));
  }
  rows.push(disclosure(ctx, more));
  if more.get() {
    if !at_limit {
      rows.extend(account(ctx, run, false));
    }
    rows.extend(rest(ctx, task, run));
  }
  Column::new()
    .id("task-facts")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Lg))
    .spacing(ExtraSpacing::Tight.role())
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(head)
    .with_children(rows)
    .into()
}

/// The account the run's latest agent started on; `at_limit` while the run is stopped at its plan limit.
fn account(ctx: &mut Ctx, run: &RunView, at_limit: bool) -> Option<Element> {
  let (_, account) = run.accounts.last()?;
  Some(
    ctx.mount_keyed::<super::run_account::RunAccount>(
      "task-fact-account",
      super::run_account::Props(*account, at_limit),
    ),
  )
}

/// The rest of the facts, while the disclosure is open.
fn rest(ctx: &mut Ctx, task: &TaskContext, run: &RunView) -> Vec<Element> {
  let mut rows = Vec::new();
  let cached = ctx.t_args(
    "summary.thisRun.cachedValue",
    [("count", compact_tokens(run.usage().cached_input))],
  );
  rows.push(fact(
    "task-fact-cached",
    &ctx.t("summary.thisRun.cached"),
    value("task-fact-cached", &cached, Look::Secondary),
  ));
  rows.extend(duration(ctx, run).map(|duration| {
    fact(
      "task-fact-duration",
      &ctx.t("summary.runDetails.duration"),
      value("task-fact-duration", &duration, Look::Secondary),
    )
  }));
  let agents = agents(run);
  if run.run.status().is_terminal() && !agents.is_empty() {
    let list = agents.join(&ctx.t("task.listSeparator"));
    rows.push(fact(
      "task-fact-agents",
      &ctx.t("summary.runDetails.agents"),
      value("task-fact-agents", &list, Look::Secondary),
    ));
  }
  rows.extend(permissions(ctx, run));
  rows.extend(delivery_rows(ctx, task.detail.queue.as_ref()));
  rows
}

/// *Account, cached input, duration, agents, permissions* and a chevron, which opens or closes them.
fn disclosure(ctx: &Ctx, more: &Signal<bool>) -> Element {
  let open = more.get();
  let toggle = more.clone();
  let glyph = if open { Icon::ChevronUp } else { Icon::ChevronDown };
  let words = ctx.t("summary.facts.more");
  pressable(move || toggle.update(|open| *open = !*open))
    .id("task-facts-more")
    .describe("a11y_name", words.clone())
    .describe("state", if open { "open" } else { "closed" })
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(value("task-facts-more-label", &words, Look::Secondary))
    .into()
}

/// "main · 4d2b8e0": the branch the run started from at the revision it started at.
fn base_value(ctx: &Ctx, run: &RunView) -> Arc<str> {
  let revision = display::short_revision(run.run.base().as_str());
  match run.run.base_branch() {
    Some(branch) => ctx.t_args(
      "summary.thisRun.baseValue",
      [("branch", branch.as_str()), ("revision", revision)],
    ),
    None => Arc::from(revision),
  }
}

/// *Checkpoint*: its revision and time, or that the run has none yet.
fn checkpoint(ctx: &Ctx, run: &RunView) -> Element {
  let (text, look) = match run.run.last_checkpoint() {
    Some(checkpoint) => (
      ctx.t_args(
        "summary.thisRun.checkpointValue",
        [
          (
            "revision",
            display::short_revision(checkpoint.revision().as_str()).to_owned(),
          ),
          ("time", display::clock(checkpoint.created_at())),
        ],
      ),
      Look::Mono,
    ),
    None => (ctx.t("summary.thisRun.noCheckpoint"), Look::Secondary),
  };
  fact(
    "task-fact-checkpoint",
    &ctx.t("summary.facts.checkpoint"),
    value("task-fact-checkpoint", &text, look),
  )
}

/// *Tokens*: "412k of 500k", in `warning` once the run used its budget up; "412k · no limit". While an agent's CLI
/// reported input but no output ([`orchester_core::run::TokenUsage::output_unreported`]), its output is unknown and
/// the count leaves it out: "412k of 500k · output not reported yet".
fn tokens(ctx: &Ctx, run: &RunView) -> Element {
  let used = compact_tokens(run.run.used().get());
  let text = match run.run.budget() {
    Some(budget) => ctx.t_args(
      "summary.runDetails.tokensValue",
      [("used", used), ("budget", compact_tokens(budget.get()))],
    ),
    None => ctx.t_args("summary.runDetails.tokensNoLimit", [("used", used)]),
  };
  let unreported = run
    .invocations
    .iter()
    .any(|invocation| invocation.usage.output_unreported());
  let text = if unreported {
    ctx.t_args("summary.runDetails.tokensOutputUnreported", [("tokens", &*text)])
  } else {
    text
  };
  let shown = value("task-fact-tokens", &text, Look::Secondary);
  let shown = if run.run.token_limit_reached() {
    shown.color(PaletteColor::Warning)
  } else {
    shown
  };
  fact("task-fact-tokens", &ctx.t("summary.facts.tokens"), shown)
}

/// The step whose agent the facts describe: the current step, or while that is a tool step, the last agent step that
/// ran.
fn agent_step(run: &RunView) -> Option<&StepView> {
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

/// "gpt-5.5 · Codex" of the agent step; a tool step's command runs on no model.
fn model(ctx: &Ctx, run: &RunView) -> Arc<str> {
  let Some(step) = agent_step(run) else {
    return ctx.t("summary.thisRun.none");
  };
  if step.tool_step {
    return ctx.t("summary.thisRun.commandTool");
  }
  match (step.model.as_deref(), step.agent.as_deref()) {
    (Some(model), Some(agent)) => ctx.t_args("summary.facts.modelValue", [("model", model), ("agent", agent)]),
    (Some(model), None) => Arc::from(model),
    (None, _) => ctx.t("summary.thisRun.none"),
  }
}

/// Where the run works: its worktree's folder, its middle cut, the whole path in its tooltip and copied by a press; or
/// when it was removed, or that its removal left its folder on disk and why (ORC-418).
fn worktree(ctx: &mut Ctx, run: &RunView, root: &Path) -> Element {
  let key = ctx.t("summary.facts.worktree");
  if let Some(left) = &run.worktree_not_removed {
    let path = ("path", left.path.as_str());
    let text = match &left.reason {
      Some(reason) => ctx.t_args(
        "summary.thisRun.worktreeNotRemoved",
        [path, ("reason", reason.as_str())],
      ),
      None => ctx.t_args("summary.thisRun.worktreeNotRemovedNoReason", [path]),
    };
    return fact(
      "task-fact-worktree",
      &key,
      value("task-fact-worktree", &text, Look::Secondary),
    );
  }
  if let Some(removed) = run.worktree_removed {
    let text = ctx.t_args("summary.thisRun.worktreeRemoved", [("time", display::clock(removed))]);
    return fact(
      "task-fact-worktree",
      &key,
      value("task-fact-worktree", &text, Look::Secondary),
    );
  }
  let path = run_worktree(root, run.id()).display().to_string();
  let clipboard = ctx.use_context::<Clipboard>().unwrap_or_default();
  let copied = path.clone();
  let shown = pressable(move || {
    if !clipboard.copy(&copied) {
      tracing::warn!("the worktree's path could not be put on the clipboard");
    }
  })
  .id("task-fact-worktree-copy")
  .flex(1.0)
  .min_width(Dimension::Px(0.0))
  .child(
    value(
      "task-fact-worktree",
      &display::shorten_path(Path::new(&path), PATH_CHARS),
      Look::Mono,
    )
    .nowrap()
    .text_overflow(TextOverflow::Elipsis),
  );
  let tip = ctx.t_args("summary.facts.worktreeTip", [("path", path)]);
  let shown = Tooltip::new("task-fact-worktree-tip", tip).mono().wrap(ctx, shown);
  fact(
    "task-fact-worktree",
    &key,
    Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(shown),
  )
}

/// "38 min · 16:14 – 16:52": from the run's first agent's start to its end.
fn duration(ctx: &Ctx, run: &RunView) -> Option<Arc<str>> {
  let (start, end) = (run.invocations.first()?.started_at, run.finished_at?);
  let minutes = (end.unix_millis() - start.unix_millis()).max(0) / MILLIS_PER_MINUTE;
  Some(ctx.t_args(
    "summary.runDetails.durationValue",
    [
      ("minutes", minutes.to_string()),
      ("start", display::clock(start)),
      ("end", display::clock(end)),
    ],
  ))
}

/// The agents of the steps that ran, in pipeline order, each once.
fn agents(run: &RunView) -> Vec<String> {
  let mut agents: Vec<String> = Vec::new();
  let ran = run.steps.iter().filter(|step| {
    !step.tool_step
      && matches!(
        step.status,
        Some(StepStatus::Running | StepStatus::Waiting(_) | StepStatus::Passed | StepStatus::Failed)
      )
  });
  for agent in ran.filter_map(|step| step.agent.as_deref()) {
    if !agents.iter().any(|known| known == agent) {
      agents.push(agent.to_owned());
    }
  }
  agents
}

/// What the current step's agent may do: "Running with · Builder" over "Edit files · 3 commands · Network off · MCP
/// asks"; nothing for a tool step, or once its profile version is gone.
fn permissions(ctx: &Ctx, run: &RunView) -> Option<Element> {
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
  let shown = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(value(
      "task-fact-permissions-lead",
      &ctx.t_args(lead_key, [("agent", agent)]),
      Look::Muted,
    ))
    .child(value("task-fact-permissions", &summary, Look::Secondary));
  Some(fact(
    "task-fact-permissions",
    &ctx.t("summary.thisRun.permissions"),
    shown,
  ))
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

#[derive(Clone, Copy, PartialEq, Eq)]
struct ActiveProps {
  active: ActiveClock,
  /// The run's time limit, in whole minutes.
  max: Option<u64>,
}

impl lurq::app::component::DevtoolsInspectable for ActiveProps {}

/// *Active time* of the run's limit, counting on while an agent works.
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
    let ActiveProps { active, max } = *ctx.props::<ActiveProps>();
    let now = self.ticker.now(ctx, active.counting());
    let time = message_text(ctx, &display::duration(active.at(now))).to_string();
    let text = match max {
      Some(minutes) => ctx.t_args(
        "summary.facts.activeOf",
        [("active", time), ("minutes", minutes.to_string())],
      ),
      None => ctx.t_args("summary.facts.activeNoLimit", [("active", time)]),
    };
    fact(
      "task-fact-active",
      &ctx.t("summary.facts.active"),
      value("task-fact-active", &text, Look::Secondary),
    )
  }
}
