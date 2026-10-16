//! The top of the Tasks list: its title, what it shows and its actions (`Header` `ogbVF`), and the workflow
//! tabs with their counts (`Workflow tabs` `bxhee`). Create multiple is not built yet: it stays in place,
//! locked, and says why under the actions (`Q9aeZ`, `Nt3Qi`). A linked project's list says where it lives under its
//! description (`Shared line` in `Xr4Wb`); once the person is no longer a member, New task is locked too (`sErKV`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Stack, Text},
  layout::Alignment,
  node::{dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::list;

use super::{
  queue::{QueueControls, QueueProps},
  shared::{self, Shared},
  view::TabCounts,
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList, TaskListTab},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    tabs::{PageTab, counted, page_tabs},
    unavailable::with_reason,
  },
};

/// The title and what the list shows, with the holds on new starts once the tasks are read (`queue`), and New task
/// and Create multiple at the end; a linked project's line under them.
pub(super) fn header(ctx: &mut Ctx, project: ProjectId, queue: Option<QueueProps>, shared: Option<&Shared>) -> Column {
  let queue = queue.map(|queue| ctx.mount_keyed::<QueueControls>(&format!("tasks-queue-{project}"), queue));
  let navigation = Navigation::of(ctx);
  let read_only = shared.and_then(|shared| shared.read_only(ctx));
  let reason = read_only.clone().unwrap_or_else(|| ctx.t("tasks.createMultipleReason"));
  let new = new_task(ctx, "tasks-new-task", project, navigation);
  let new = match read_only {
    Some(reason) => new.unavailable(reason),
    None => new,
  };
  let line = shared.map(|shared| shared::line(ctx, project, shared));
  let create_multiple = ActionButton::new(
    "tasks-create-multiple",
    ButtonKind::Secondary,
    ctx.t("tasks.createMultiple"),
  )
  .icon(Icon::ListPlus)
  .unavailable(reason.clone());
  let actions = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(queue)
    .child(create_multiple)
    .child(new);
  let row = Row::new()
    .id("tasks-header")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().bottom(SpacingSize::Xs))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::End)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Snug.role())
        .child(
          Text::new(&ctx.t("tasks.title"))
            .id("tasks-title")
            .variant(ExtraTypography::Display.role()),
        )
        .child(
          Text::new(&ctx.t("tasks.description"))
            .variant(TypographyStyle::Body)
            .color(PaletteColor::TextSecondary),
        ),
    )
    .child(with_reason(actions, "tasks-create-multiple-reason", &reason));
  // The line takes the page's width under the title and the actions, which the holds on new starts widen beyond
  // `Xr4Wb`'s.
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(row)
    .with_children(line)
}

/// New task, which opens the New task form.
pub(super) fn new_task(ctx: &Ctx, id: &str, project: ProjectId, navigation: Navigation) -> ActionButton {
  ActionButton::new(id.to_owned(), ButtonKind::Primary, ctx.t("tasks.newTask"))
    .icon(Icon::Plus)
    .on_click(move || navigation.go(Route::Project(project, ProjectPage::NewTask)))
}

/// The workflow tabs, each with how many tasks it holds once the tasks are read, `—` until then. Another
/// tab keeps the search; the task Create just added is no longer pointed out there.
pub(super) fn tabs(ctx: &mut Ctx, project: ProjectId, list: &TaskList, counts: Option<TabCounts>) -> Stack {
  let navigation = Navigation::of(ctx);
  let unknown = ctx.t("tasks.tabs.unknownCount");
  let tabs: Vec<PageTab> = TaskListTab::ALL
    .into_iter()
    .map(|tab| {
      let word = ctx.t(label_key(tab));
      let label = match counts {
        Some(counts) => counted(ctx, &word, counts.get(tab)),
        None => ctx.t_args("ui.tabCount", [("label", word), ("count", unknown.clone())]),
      };
      let to = TaskList::new(tab).with_search(list.search().unwrap_or_default());
      let navigation = navigation.clone();
      PageTab::new(
        format!("tasks-tab-{}", tab.name()),
        label,
        tab == list.tab(),
        move || navigation.go(Route::Project(project, ProjectPage::Tasks(to.clone()))),
      )
    })
    .collect();
  page_tabs(ctx, "tasks-tabs", tabs)
}

/// The workspace's tab of the same tasks.
pub(super) const fn workspace_tab(tab: TaskListTab) -> list::TaskTab {
  match tab {
    TaskListTab::All => list::TaskTab::All,
    TaskListTab::NeedsYou => list::TaskTab::NeedsYou,
    TaskListTab::Backlog => list::TaskTab::Backlog,
    TaskListTab::Todo => list::TaskTab::Todo,
    TaskListTab::InProgress => list::TaskTab::InProgress,
    TaskListTab::InReview => list::TaskTab::InReview,
    TaskListTab::Done => list::TaskTab::Done,
  }
}

pub(super) const fn label_key(tab: TaskListTab) -> &'static str {
  match tab {
    TaskListTab::All => "tasks.tabs.all",
    TaskListTab::NeedsYou => "tasks.tabs.needsYou",
    TaskListTab::Backlog => "tasks.tabs.backlog",
    TaskListTab::Todo => "tasks.tabs.todo",
    TaskListTab::InProgress => "tasks.tabs.inProgress",
    TaskListTab::InReview => "tasks.tabs.inReview",
    TaskListTab::Done => "tasks.tabs.done",
  }
}
