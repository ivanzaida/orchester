//! The list's views (`Views` `ANTgi` in `Tasks · redesign` `Lw1Gb`): Active, Backlog, Done and All, each with how many
//! tasks it holds as searched and filtered once they are read, `—` until then; 32 high, 20 apart, on a 1 `border`
//! line. The current view's label is 13/500 `text_primary` over a 2 `accent` underline, the others' 13
//! `text_secondary`; each count follows its label 6 after it in 12 `text_muted`. Another view keeps the search; the
//! task a Create just added is no longer pointed out there. At the line's end, while new starts are held, the chip
//! that says so with its Release (`queue`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Row, Spacer, Stack, Text},
  layout::{Alignment, StackAlignment},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::views::ListView;

use super::cache::ViewCounts;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList, TaskListTab},
  theme::{ExtraSpacing, ExtraTypography},
  ui::pressable::opening,
};

/// A view's height (`Active` `Decmu`).
const VIEW_HEIGHT: f32 = 32.0;

/// The views with their counts, `None` until the tasks are read, and `end` at the line's end.
pub(super) fn views(
  ctx: &mut Ctx,
  (project, list): (ProjectId, &TaskList),
  counts: Option<ViewCounts>,
  end: Option<Element>,
) -> Stack {
  let navigation = Navigation::of(ctx);
  let tabs: Vec<Element> = TaskListTab::ALL
    .into_iter()
    .map(|tab| view(ctx, (&navigation, project, list), counts, tab))
    .collect();
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  Stack::new()
    .id("tasks-views")
    .width(Dimension::Pct(100.0))
    .stack_align(StackAlignment::BottomStart)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(line))
        .background(PaletteColor::Border),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tabs.role())
        .align_items(Alignment::Center)
        .with_children(tabs)
        .child(Spacer::new().flex(1.0))
        .with_children(end),
    )
}

/// One view, which opens the list on it.
fn view(
  ctx: &Ctx,
  (navigation, project, list): (&Navigation, ProjectId, &TaskList),
  counts: Option<ViewCounts>,
  tab: TaskListTab,
) -> Element {
  let id = format!("tasks-view-{}", tab.name());
  let current = tab == list.tab();
  let word = ctx.t(label_key(tab));
  let count = counts.map_or_else(|| ctx.t("tasks.views.unknownCount").to_string(), |counts| counts.get(tab).to_string());
  let (style, color) = if current {
    (ExtraTypography::Strong.role(), PaletteColor::TextPrimary)
  } else {
    (TypographyStyle::Body, PaletteColor::TextSecondary)
  };
  let to = TaskList::new(tab).with_search(list.search().unwrap_or_default());
  let button = opening(navigation.to(Route::Project(project, ProjectPage::Tasks(to))))
    .id(id.clone())
    .height(Dimension::Px(VIEW_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
    .spacing(ExtraSpacing::Snug.role())
    .describe("a11y_name", ctx.t_args("ui.tabCount", [("label", &*word), ("count", &count)]))
    .describe("state", if current { "current" } else { "enabled" })
    .child(
      Text::new(&word)
        .id(format!("{id}-label"))
        .variant(style)
        .color(color)
        .nowrap(),
    )
    .child(
      Text::new(&count)
        .id(format!("{id}-count"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
  if current {
    button
      .border_bottom(Border::inside(BorderSize::Md, PaletteColor::Accent))
      .into()
  } else {
    button.into()
  }
}

/// The workspace's view of the same tasks.
pub(super) const fn list_view(tab: TaskListTab) -> ListView {
  match tab {
    TaskListTab::Active => ListView::Active,
    TaskListTab::Backlog => ListView::Backlog,
    TaskListTab::Done => ListView::Done,
    TaskListTab::All => ListView::All,
  }
}

pub(super) const fn label_key(tab: TaskListTab) -> &'static str {
  match tab {
    TaskListTab::Active => "tasks.views.active",
    TaskListTab::Backlog => "tasks.views.backlog",
    TaskListTab::Done => "tasks.views.done",
    TaskListTab::All => "tasks.views.all",
  }
}
