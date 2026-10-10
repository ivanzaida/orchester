//! The bar's right side (`sMpGe`): the project's counts — *Agents 3/4* (agents at work of the runner's
//! agent slots, a dash unless the runner is online and reported them), *Queued 2*, *4 need you* (a hand,
//! amber while anything waits; 0 stays visible), *3.8M / 20M today* (today's tokens against the
//! project's daily budget) — then the app's MCP server, whether agents may control the window, and the
//! version. Counts are plain numbers, one space after their label; a dash only means "not known yet".
//! Agents opens Tasks, Queued and Needs you the Overview's list of them, the tokens Usage.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Button, Row},
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::tokens::compact_tokens;
use orchester_workspace::{runner::state::RunnerState, tasks::data::ProjectTasks};

use super::{
  facts::{BarAction, Dispatch, ProjectFacts},
  item::{ITEM_HEIGHT, dropping, give_way, glyph, item, label, pressed},
};
use crate::{
  mcp::McpStatus,
  routes::{OverviewTab, ProjectPage, Route, TaskList},
  theme::ExtraSpacing,
  ui::{icon::Icon, pressable::pressable},
};

type Press = Arc<dyn Fn() + Send + Sync>;

/// The project's counts, in the bar's order. While the tasks are not known, Queued and Needs you show a
/// dash, so the bar does not move once they are. The three counts are one part of the bar whose labels
/// give way together (lurq drops every droppable child of a line at once when the line collapses), so
/// each count is laid out as pieces of that line — its icon, its words and its value — each pressing the
/// same way.
pub fn counts(ctx: &Ctx, project: &ProjectFacts, dispatch: &Dispatch) -> Vec<Element> {
  let go = |page: ProjectPage| -> Press {
    let (dispatch, route) = (dispatch.clone(), Route::Project(project.id, page));
    Arc::new(move || dispatch(BarAction::Go(route.clone())))
  };
  let unknown = ctx.t("shell.status.unknown").to_string();
  let agents = agents(project).unwrap_or_else(|| unknown.clone());
  let tasks = project.tasks.as_ref();
  let queued = tasks.map_or_else(
    || unknown.clone(),
    |tasks| orchester_workspace::tasks::list::queued(&tasks.tasks, &tasks.phases).to_string(),
  );
  let waiting = tasks.map(ProjectTasks::needs_you);
  let waiting_value = waiting.map_or_else(|| unknown.clone(), |count| count.to_string());
  let needs_words = ctx.t(if waiting == Some(1) {
    "shell.status.needsYou.one"
  } else {
    "shell.status.needsYou.other"
  });
  let counts = [
    Count {
      id: "status-agents",
      icon: Icon::Bot,
      color: PaletteColor::TextMuted,
      words: (&ctx.t("shell.status.agents"), true),
      value: &agents,
      press: go(ProjectPage::Tasks(TaskList::default())),
    },
    Count {
      id: "status-queued",
      icon: Icon::Clock3,
      color: PaletteColor::TextMuted,
      words: (&ctx.t("shell.status.queued"), true),
      value: &queued,
      press: go(ProjectPage::Overview(OverviewTab::Queued)),
    },
    Count {
      id: "status-needs-you",
      icon: Icon::Hand,
      color: if waiting.is_some_and(|count| count > 0) {
        PaletteColor::Warning
      } else {
        PaletteColor::TextMuted
      },
      words: (&needs_words, false),
      value: &waiting_value,
      press: go(ProjectPage::Overview(OverviewTab::NeedsYou)),
    },
  ];
  let group = counts.into_iter().flat_map(Count::pieces).fold(
    Row::new()
      .id("status-counts")
      .flex_shrink(1.0)
      .shrink_order(give_way::COUNT_LABELS)
      .shrink_limit(ShrinkLimit::Content)
      .align_items(Alignment::Center),
    Row::child,
  );
  let usage = go(ProjectPage::Usage);
  vec![group.into(), tokens(ctx, project, &unknown, move || usage())]
}

/// One count: *Agents 3/4* (its words before its value) or *4 need you* (after it).
struct Count<'a> {
  id: &'static str,
  icon: Icon,
  color: PaletteColor,
  /// The words, and whether they come before the value.
  words: (&'a str, bool),
  value: &'a str,
  press: Press,
}

impl Count<'_> {
  /// The icon (8 before it, 6 after it), the words (dropping out, 3 from the value) and the value, 8 to
  /// the next item. The icon's piece has the count's element id, the words' `<id>-label`, the value's
  /// text `<id>-value`.
  fn pieces(self) -> Vec<Element> {
    let (words, before) = self.words;
    let (fine, snug, sm) = (ExtraSpacing::Fine.role(), ExtraSpacing::Snug.role(), SpacingSize::Sm);
    let icon = self.piece(self.id.to_owned(), Padding::new().left(sm).right(snug));
    let icon = icon.child(glyph(self.icon, self.color.clone()));
    let words = |right: SpacingSize| {
      self
        .piece(format!("{}-label", self.id), Padding::new().right(right))
        .flex_shrink(1.0)
        .shrink_limit(ShrinkLimit::Drop)
        .child(label(words, self.color.clone()))
    };
    let value = |right: SpacingSize| {
      self
        .piece(format!("{}-count", self.id), Padding::new().right(right))
        .child(label(self.value, self.color.clone()).id(format!("{}-value", self.id)))
    };
    if before {
      vec![icon.into(), words(fine).into(), value(sm).into()]
    } else {
      vec![icon.into(), value(fine).into(), words(sm).into()]
    }
  }

  fn piece(&self, id: String, padding: Padding) -> Button {
    let press = self.press.clone();
    pressable(move || press())
      .id(id)
      .height(Dimension::Px(ITEM_HEIGHT))
      .padding(padding)
      .align_items(Alignment::Center)
  }
}

/// *3/4*: agents at work of the agent slots, as the runner reports them; not known (a dash) unless the
/// runner is online, or stopping while the window closes, and has reported.
fn agents(project: &ProjectFacts) -> Option<String> {
  match (&project.runner.state, &project.runner.work) {
    // While the window closes the runner stops, and its last report still counts.
    (RunnerState::Online | RunnerState::Stopping, Some(work)) => {
      Some(format!("{}/{}", work.agents_working(), work.agent_slots))
    }
    _ => None,
  }
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
        .shrink_order(give_way::WINDOW_CONTROL)
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
