//! The top of the Tasks list: its title, what it shows and its actions (`Header` `ogbVF`), and the workflow
//! tabs with their counts (`Workflow tabs` `bxhee`). Create multiple is not built yet: it stays in place,
//! locked, and says why under the actions (`Q9aeZ`, `Nt3Qi`).

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
use orchester_workspace::tasks::{data::ProjectTasks, list};

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

/// The title and what the list shows, with New task and Create multiple at the end.
pub(super) fn header(ctx: &mut Ctx, project: ProjectId) -> Row {
  let navigation = Navigation::of(ctx);
  let reason = ctx.t("tasks.createMultipleReason");
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
    .child(create_multiple)
    .child(new_task(ctx, "tasks-new-task", project, navigation));
  Row::new()
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
    .child(with_reason(actions, "tasks-create-multiple-reason", &reason))
}

/// New task, which opens the New task form.
pub(super) fn new_task(ctx: &Ctx, id: &str, project: ProjectId, navigation: Navigation) -> ActionButton {
  ActionButton::new(id.to_owned(), ButtonKind::Primary, ctx.t("tasks.newTask"))
    .icon(Icon::Plus)
    .on_click(move || navigation.go(Route::Project(project, ProjectPage::NewTask)))
}

/// The workflow tabs, each with how many tasks it holds once the tasks are read, `—` until then. Another
/// tab keeps the search; the task Create just added is no longer pointed out there.
pub(super) fn tabs(ctx: &mut Ctx, project: ProjectId, list: &TaskList, tasks: Option<&ProjectTasks>) -> Stack {
  let navigation = Navigation::of(ctx);
  let unknown = ctx.t("tasks.tabs.unknownCount");
  let tabs: Vec<PageTab> = TaskListTab::ALL
    .into_iter()
    .map(|tab| {
      let word = ctx.t(label_key(tab));
      let label = match tasks {
        Some(tasks) => counted(ctx, &word, count(tasks, tab)),
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

/// How many tasks `tab` holds, whatever the search and the pipeline filter.
pub(super) fn count(tasks: &ProjectTasks, tab: TaskListTab) -> usize {
  list::rows(&tasks.tasks, &tasks.pipelines, &tasks.phases, workspace_tab(tab)).len()
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
