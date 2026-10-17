//! The Overview's tasks (`Tasks` `xMAkd`): on a panel, the tabs Running, Needs you, Queued and Failed
//! with how many lines each holds, the newest tasks of the chosen one (`row`) and after them the lines
//! that are no task's (the memory records waiting, in Needs you `mEvfK`), and how many tasks are
//! active and completed with the way to the whole list (`Footer` `nSuzk`). While the tasks are read the
//! tabs count `—` over a reading line (`Reading` `X59I5`). A tab is the route's, so Back returns to it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::{board::BoardTab, data::ProjectTasks, overview::footer};

use super::row::{project_row, row};
use crate::{
  navigation::Navigation,
  routes::{OverviewTab, ProjectPage, Route, TaskList, TaskListTab},
  theme::{ExtraSpacing, ExtraTypography, OverviewSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card_frame},
    icon::Icon,
    message::message_text,
    state::loading,
    tabs::{PageTab, counted, page_tabs},
  },
};

/// Tasks a tab shows here; the Tasks list has them all.
const SHOWN: usize = 6;

/// The title over the panel; `tasks` is `None` while they are read. `project` is the project's id and name.
pub(super) fn section(
  ctx: &mut Ctx,
  (project, name): (ProjectId, &str),
  tab: OverviewTab,
  tasks: Option<&ProjectTasks>,
) -> Column {
  let panel = card_frame("overview-task-list", CardSurface::Panel).child(tabs(ctx, project, tab, tasks));
  let panel = match tasks {
    None => panel.child(reading(
      &ctx.t_args("overview.tasks.reading", [("project", name)]),
      "overview-tasks-reading",
    )),
    Some(tasks) => {
      let separator = ctx.t("overview.listSeparator");
      let rows = board_tab(tab).rows(tasks, SHOWN, &separator);
      let others = board_tab(tab).project_rows(tasks, name);
      let mut shown: Vec<Element> = rows.iter().map(|shown| row(ctx, project, shown).into()).collect();
      shown.extend(others.iter().map(|other| project_row(ctx, other).into()));
      if shown.is_empty() {
        shown.push(empty_tab(ctx, tab).into());
      }
      panel.with_children(shown).child(footer_row(ctx, project, tasks))
    }
  };
  Column::new()
    .id("overview-tasks")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(Text::new(&ctx.t("overview.tasks.title")).variant(TypographyStyle::Title))
    .child(panel)
}

/// The workspace's tab of the route's.
const fn board_tab(tab: OverviewTab) -> BoardTab {
  match tab {
    OverviewTab::Running => BoardTab::Running,
    OverviewTab::NeedsYou => BoardTab::NeedsYou,
    OverviewTab::Queued => BoardTab::Queued,
    OverviewTab::Failed => BoardTab::Failed,
  }
}

const fn label_key(tab: OverviewTab) -> &'static str {
  match tab {
    OverviewTab::Running => "overview.tabs.running",
    OverviewTab::NeedsYou => "overview.tabs.needsYou",
    OverviewTab::Queued => "overview.tabs.queued",
    OverviewTab::Failed => "overview.tabs.failed",
  }
}

/// The tabs on their line, 16 in from the panel's sides, each with its count once the tasks are read.
fn tabs(ctx: &mut Ctx, project: ProjectId, current: OverviewTab, tasks: Option<&ProjectTasks>) -> Column {
  let navigation = Navigation::of(ctx);
  let unknown = ctx.t("overview.unknown");
  let tabs: Vec<PageTab> = OverviewTab::ALL
    .into_iter()
    .map(|tab| {
      let word = ctx.t(label_key(tab));
      let label = match tasks {
        Some(tasks) => counted(ctx, &word, board_tab(tab).count(tasks)),
        None => ctx.t_args("ui.tabCount", [("label", word), ("count", unknown.clone())]),
      };
      PageTab::to(
        format!("overview-tab-{}", tab.name()),
        label,
        tab == current,
        navigation.to(Route::Project(project, ProjectPage::Overview(tab))),
      )
    })
    .collect();
  // The panel's line runs under the tabs from edge to edge; theirs, inside the padding, covers part of it.
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Lg))
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(page_tabs(ctx, "overview-tabs", tabs))
}

/// The loader and what is read, in the panel (`Reading` `X59I5`).
pub(super) fn reading(what: &str, id: &str) -> Row {
  loading(id, what).padding(Padding::symmetric(SpacingSize::Lg, OverviewSpacing::Reading.role()))
}

/// The tab holds no task.
fn empty_tab(ctx: &Ctx, tab: OverviewTab) -> Row {
  let word = ctx.t(label_key(tab));
  Row::new()
    .id("overview-tasks-empty")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, OverviewSpacing::Reading.role()))
    .justify(Justify::Center)
    .child(
      Text::new(&ctx.t_args("overview.tasks.emptyTab", [("tab", word)]))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
}

/// Active and completed tasks, and *View all tasks*.
fn footer_row(ctx: &mut Ctx, project: ProjectId, tasks: &ProjectTasks) -> Row {
  let navigation = Navigation::of(ctx);
  let all = ActionButton::new("overview-view-all", ButtonKind::Ghost, ctx.t("overview.tasks.viewAll"))
    .icon(Icon::ArrowRight)
    .opens(navigation.to(Route::Project(
      project,
      ProjectPage::Tasks(TaskList::new(TaskListTab::All)),
    )));
  Row::new()
    .id("overview-footer")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .left(ExtraSpacing::Inset.role()),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&message_text(ctx, &footer(&tasks.tasks, &tasks.phases)))
        .id("overview-footer-count")
        .variant(ExtraTypography::Note.role())
        .flex(1.0),
    )
    .child(all)
}
