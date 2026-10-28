//! The bar's right side (`sMpGe`): the project's counts — *Agents 3/4* (agents at work of the runner's
//! agent slots, a dash unless the runner is online and reported them), *Queued 2*, *4 need you* (a hand,
//! amber while anything waits; 0 stays visible) — then the app's MCP server, whether agents may control the
//! window, and the version. Counts are plain numbers, one space after their label; a dash only means "not known
//! yet". Agents opens Tasks, Queued and Needs you the Overview's list of them. Today's tokens show once, in the
//! sidebar's usage card (`Hub pages` `ZRBS5`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Button, Row},
  core::ElementRef,
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{runner::state::RunnerState, tasks::data::ProjectTasks};

use super::{
  facts::{Dispatch, ProjectFacts, go},
  item::{ITEM_HEIGHT, dropping, give_way, glyph, item, label},
  mcp::mcp_item,
};
use crate::{
  mcp::McpStatus,
  routes::{OverviewTab, ProjectPage, Route, TaskList},
  theme::ExtraSpacing,
  ui::{icon::Icon, pressable::Press},
};

/// The project's counts, in the bar's order. While the tasks are not known, Queued and Needs you show a
/// dash, so the bar does not move once they are. The three counts are one part of the bar whose labels
/// give way together (lurq drops every droppable child of a line at once when the line collapses), so
/// each count is laid out as pieces of that line — its icon, its words and its value — each pressing the
/// same way.
pub fn counts(ctx: &Ctx, project: &ProjectFacts, dispatch: &Dispatch) -> Vec<Element> {
  let open = |page: ProjectPage| go(dispatch, Route::Project(project.id, page));
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
      press: open(ProjectPage::Tasks(TaskList::default())),
    },
    Count {
      id: "status-queued",
      icon: Icon::Clock3,
      color: PaletteColor::TextMuted,
      words: (&ctx.t("shell.status.queued"), true),
      value: &queued,
      press: open(ProjectPage::Overview(OverviewTab::Queued)),
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
      press: open(ProjectPage::Overview(OverviewTab::NeedsYou)),
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
  vec![group.into()]
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
    self
      .press
      .clone()
      .button()
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

/// The app's MCP server (it never gives way), whether agents may control the window, and the version
/// (the first to drop out).
pub fn app(ctx: &mut Ctx, (mcp, bar): (&McpStatus, &ElementRef), dispatch: &Dispatch) -> Vec<Element> {
  let mut items = Vec::new();
  if mcp.window_control() {
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
  items.push(mcp_item(ctx, mcp, dispatch, bar));
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
