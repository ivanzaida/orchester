//! The bar's right side (`sMpGe`): the project's counts — *Agents 3/4* (agents at work of the runner's
//! agent slots, a dash while that is not known), *Queued 2*, *4 need you* (a hand, amber while anything
//! waits; 0 stays visible), *3.8M / 20M today* (today's tokens against the project's daily budget) — then
//! the app's MCP server, whether agents may control the window, and the version. Counts are plain
//! numbers; a dash only means "not known yet". Agents opens Tasks, Queued and Needs you the Overview's
//! list of them, the tokens Usage.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  layout::layout_kind::ShrinkLimit,
  node::Element,
};
use orchester_core::tokens::compact_tokens;
use orchester_workspace::runner::state::RunnerState;

use super::{
  facts::{BarAction, Dispatch, ProjectFacts},
  item::{dropping, give_way, glyph, item, label, pressed},
};
use crate::{
  mcp::McpStatus,
  routes::{OverviewTab, ProjectPage, Route, TaskList},
  shell::needs_you::count_words,
  ui::icon::Icon,
};

/// The project's counts, in the bar's order.
pub fn counts(ctx: &Ctx, project: &ProjectFacts, dispatch: &Dispatch) -> Vec<Element> {
  let go = |page: ProjectPage| {
    let (dispatch, route) = (dispatch.clone(), Route::Project(project.id, page));
    move || dispatch(BarAction::Go(route.clone()))
  };
  let unknown = ctx.t("shell.status.unknown").to_string();
  let agents = agents(project).unwrap_or_else(|| unknown.clone());
  let mut items = vec![
    pressed("status-agents", go(ProjectPage::Tasks(TaskList::default())))
      .flex_shrink(1.0)
      .shrink_order(give_way::COUNT_LABELS)
      .shrink_limit(ShrinkLimit::Content)
      .child(glyph(Icon::Bot, PaletteColor::TextMuted))
      .child(dropping(&ctx.t("shell.status.agents"), PaletteColor::TextMuted))
      .child(label(&agents, PaletteColor::TextMuted).id("status-agents-value"))
      .into(),
  ];
  if let Some(tasks) = &project.tasks {
    let queued = orchester_workspace::tasks::list::queued(&tasks.tasks, &tasks.phases);
    items.push(
      pressed("status-queued", go(ProjectPage::Overview(OverviewTab::Queued)))
        .flex_shrink(1.0)
        .shrink_order(give_way::COUNT_LABELS)
        .shrink_limit(ShrinkLimit::Content)
        .child(glyph(Icon::Clock3, PaletteColor::TextMuted))
        .child(dropping(&ctx.t("shell.status.queued"), PaletteColor::TextMuted))
        .child(label(&queued.to_string(), PaletteColor::TextMuted).id("status-queued-value"))
        .into(),
    );
    items.push(needs_you(
      ctx,
      tasks.needs_you(),
      go(ProjectPage::Overview(OverviewTab::NeedsYou)),
    ));
  }
  items.push(tokens(ctx, project, &unknown, go(ProjectPage::Usage)));
  items
}

/// *3/4*: agents at work of the agent slots. While the runner works, its own report; while it is down,
/// none is at work of the project's slots; while it starts or stops, or before it reported, not known.
fn agents(project: &ProjectFacts) -> Option<String> {
  let runner = &project.runner;
  let (working, slots) = match (&runner.state, &runner.work) {
    (RunnerState::Online, Some(work)) => (work.agents_working(), work.agent_slots),
    (RunnerState::Failed { .. } | RunnerState::Reconnecting { .. } | RunnerState::Stopped, _) => {
      (0, project.usage?.limits.agent_slots().get())
    }
    _ => return None,
  };
  Some(format!("{working}/{slots}"))
}

/// *4 need you*: a hand and the count, in amber while anything waits; its words give way.
fn needs_you(ctx: &Ctx, count: usize, press: impl Fn() + Send + Sync + 'static) -> Element {
  let color = if count > 0 {
    PaletteColor::Warning
  } else {
    PaletteColor::TextMuted
  };
  let words = ctx.t(if count == 1 {
    "shell.status.needsYou.one"
  } else {
    "shell.status.needsYou.other"
  });
  pressed("status-needs-you", press)
    .describe("count", count.to_string())
    .describe("a11y_name", count_words(ctx, count))
    .flex_shrink(1.0)
    .shrink_order(give_way::COUNT_LABELS)
    .shrink_limit(ShrinkLimit::Content)
    .child(glyph(Icon::Hand, color.clone()))
    .child(label(&count.to_string(), color.clone()).id("status-needs-you-value"))
    .child(dropping(&words, color))
    .into()
}

/// *3.8M / 20M today*; it drops out whole after the version.
fn tokens(ctx: &Ctx, project: &ProjectFacts, unknown: &str, press: impl Fn() + Send + Sync + 'static) -> Element {
  let (used, budget) = project.usage.map_or_else(
    || (unknown.to_owned(), unknown.to_owned()),
    |usage| {
      (
        compact_tokens(usage.today.get()),
        compact_tokens(usage.limits.daily_budget().get()),
      )
    },
  );
  let text = ctx.t_args("shell.status.tokens", [("used", used), ("budget", budget)]);
  pressed("status-tokens", press)
    .flex_shrink(1.0)
    .shrink_order(give_way::TOKENS)
    .shrink_limit(ShrinkLimit::Drop)
    .child(glyph(Icon::Coins, PaletteColor::TextMuted))
    .child(label(&text, PaletteColor::TextMuted).id("status-tokens-value"))
    .into()
}

/// The app's MCP server (it never gives way), whether agents may control the window, and the version
/// (the first to drop out).
pub fn app(ctx: &Ctx, mcp: McpStatus) -> Vec<Element> {
  let mut items = Vec::new();
  if let McpStatus::Listening {
    window_control: true, ..
  } = mcp
  {
    items.push(
      item("status-window-control")
        .flex_shrink(1.0)
        .shrink_order(give_way::COUNT_LABELS)
        .shrink_limit(ShrinkLimit::Content)
        .child(glyph(Icon::MousePointerClick, PaletteColor::TextMuted))
        .child(dropping(&ctx.t("shell.status.windowControl"), PaletteColor::TextMuted))
        .into(),
    );
  }
  let mcp = match mcp {
    McpStatus::Listening { port, .. } => ctx.t_args("shell.status.mcpListening", [("port", port)]),
    McpStatus::Unavailable => ctx.t("shell.status.mcpUnavailable"),
  };
  items.push(
    item("status-mcp")
      .child(glyph(Icon::RadioTower, PaletteColor::TextMuted))
      .child(label(&mcp, PaletteColor::TextMuted))
      .into(),
  );
  items.push(version(ctx));
  items
}

fn version(ctx: &Ctx) -> Element {
  let version = ctx.t_args("shell.status.version", [("version", env!("CARGO_PKG_VERSION"))]);
  item("status-version")
    .flex_shrink(1.0)
    .shrink_order(give_way::VERSION)
    .shrink_limit(ShrinkLimit::Drop)
    .child(glyph(Icon::Sparkles, PaletteColor::TextMuted))
    .child(label(&version, PaletteColor::TextMuted))
    .into()
}
