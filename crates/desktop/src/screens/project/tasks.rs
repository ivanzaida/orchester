//! Tasks (02.03 `J9467`): the project's backlog with its tabs. All tasks and Drafts work;
//! queueing, review and completion do not exist yet, so their tabs are unavailable, and so are
//! selecting tasks, running them, creating several at once, searching, filtering and sorting.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, VirtualizedList},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use super::{
  ProjectNav, TasksState, page,
  task_filters::{FilterProps, Filters, Sort, TaskFilters},
  task_rows::{QueueRow, table_heading},
  task_selection::{SelectedIds, SelectionBar, SelectionProps, TaskSelection},
};
use crate::{
  navigation::route::Page,
  tasks::{
    data::ProjectTasks,
    list::{self, TaskTab},
  },
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    button::ActionButton,
    icon::Icon,
    message_text,
    panel::RecoveryPanel,
    tab::{Tab, tab_row},
  },
};

/// Topbar and its buttons, tab row, filter widths, selection bar and footer (`is9Lk`,
/// `gWN2C`, `YGDQj`, `XxplC`, `d5tAIZ`, `OjUxo`, `oYw3J`, `Bpth7`).
const TOPBAR_HEIGHT: f32 = 64.0;
const TOPBAR_BUTTON_HEIGHT: f32 = 36.0;
const CREATE_MULTIPLE_WIDTH: f32 = 142.0;
const NEW_TASK_WIDTH: f32 = 116.0;
const TAB_ROW_HEIGHT: f32 = 42.0;
const FOOTER_HEIGHT: f32 = 30.0;

#[derive(Clone, Debug, PartialEq)]
pub struct TasksProps {
  pub root: PathBuf,
  pub project: Arc<str>,
  pub tab: TaskTab,
  pub tasks: TasksState,
}

impl lurq::app::component::DevtoolsInspectable for TasksProps {}

pub struct TasksScreen {
  filter: Signal<Filters>,
  sort: Signal<Sort>,
  selected: Signal<SelectedIds>,
  nav: ProjectNav,
}

impl Component for TasksScreen {
  type Props = TasksProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      filter: ctx.signal(Filters::default()),
      sort: ctx.signal(Sort::default()),
      selected: ctx.signal(SelectedIds::default()),
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TasksProps>().clone();
    let nav = self.nav.clone();
    let tab = props.tab;
    ctx.provide(TaskSelection(self.selected.clone()));
    let body = match &props.tasks {
      TasksState::Loading => Column::new().into(),
      TasksState::Failed(error) => {
        let nav = nav.clone();
        centered(
          Column::new()
            .spacing(SpacingSize::Sm)
            .child(RecoveryPanel::new(
              Icon::AlertCircle,
              &ctx.t("tasks.loadFailed"),
              &message_text(ctx, error),
            ))
            .child(
              ActionButton::new(ctx.t("common.retry"))
                .on_click(move || nav.tasks_changed())
                .render(ctx),
            )
            .into(),
        )
      }
      TasksState::Ready(tasks) if tasks.tasks.is_empty() => Column::new()
        .flex(1.0)
        .child(
          ActionButton::new(ctx.t("tasks.tags.manage"))
            .id("project-tags-manage")
            .link(nav.link(Page::ProjectTags))
            .render(ctx),
        )
        .child(empty(ctx, &nav))
        .into(),
      TasksState::Ready(tasks) => self.library(ctx, tasks, tab, &props.root),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(topbar(ctx, &props.project, &nav))
      .child(page::organization_heading(
        &ctx.t("tasks.title"),
        &ctx.t("tasks.description"),
      ))
      .child(self.tabs(ctx, tab, props.tasks.ready()))
      .child(body)
  }
}

impl TasksScreen {
  fn tabs(&self, ctx: &Ctx, selected: TaskTab, tasks: Option<&ProjectTasks>) -> Element {
    let tabs = TaskTab::ALL.into_iter().map(|tab| {
      let item = Tab::new(tab_id(tab), ctx.t(tab.label_key()), Dimension::Px(TAB_ROW_HEIGHT))
        .min_width(Dimension::Px(tab_width(tab)));
      if !tab.is_available() {
        return item;
      }
      item
        .count(tasks.and_then(|tasks| tab.count(&tasks.tasks)))
        .selected(tab == selected)
        .link(self.nav.link(Page::Tasks(tab)))
    });
    tab_row(
      Dimension::Px(TAB_ROW_HEIGHT),
      SpacingSize::Sm,
      Some(SpacingSize::Section),
      tabs,
    )
  }
}

/// Tab widths in the design (`Iefue`, `WGkQY`, `vfoPn`, `gDolJ`, `fwdrp` in `XxplC`).
const fn tab_width(tab: TaskTab) -> f32 {
  match tab {
    TaskTab::All | TaskTab::Todo | TaskTab::Done => 48.0,
    TaskTab::Backlog => 72.0,
    TaskTab::NeedsYou | TaskTab::InProgress | TaskTab::InReview => 96.0,
  }
}
const fn tab_id(tab: TaskTab) -> &'static str {
  match tab {
    TaskTab::All => "tasks-tab-all",
    TaskTab::NeedsYou => "tasks-tab-needs-you",
    TaskTab::Backlog => "tasks-tab-backlog",
    TaskTab::Todo => "tasks-tab-todo",
    TaskTab::InProgress => "tasks-tab-in-progress",
    TaskTab::InReview => "tasks-tab-in-review",
    TaskTab::Done => "tasks-tab-done",
  }
}

fn topbar(ctx: &Ctx, project: &str, nav: &ProjectNav) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOPBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t_args("tasks.breadcrumb", [("project", project)])).variant(TypographyStyle::Description))
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new(ctx.t("tasks.createMultiple"))
        .icon(Icon::ListPlus)
        .id("create-multiple")
        .size(
          Dimension::Px(CREATE_MULTIPLE_WIDTH),
          Dimension::Px(TOPBAR_BUTTON_HEIGHT),
        )
        .render(ctx),
    )
    .child(
      ActionButton::new(ctx.t("overview.newTask"))
        .icon(Icon::Plus)
        .id("tasks-new-task")
        .size(Dimension::Px(NEW_TASK_WIDTH), Dimension::Px(TOPBAR_BUTTON_HEIGHT))
        .link(nav.link(Page::NewTask))
        .render(ctx),
    )
    .into()
}

fn body() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Stack.role())
}

fn centered(content: Element) -> Element {
  body()
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}

/// No tasks in the project yet.
fn empty(ctx: &Ctx, nav: &ProjectNav) -> Element {
  let action = ActionButton::new(ctx.t("overview.newTask"))
    .primary()
    .icon(Icon::Plus)
    .id("empty-new-task")
    .link(nav.link(Page::NewTask))
    .render(ctx);
  centered(
    RecoveryPanel::new(
      Icon::CircleCheck,
      &ctx.t("tasks.emptyTitle"),
      &ctx.t("tasks.emptyDetail"),
    )
    .action(action)
    .into(),
  )
}

impl TasksScreen {
  fn library(&self, ctx: &mut Ctx, tasks: &ProjectTasks, tab: TaskTab, root: &std::path::Path) -> Element {
    let filter = self.filter.get().0;
    let sort = self.sort.get().0;
    let mut rows: Vec<_> = list::rows(&tasks.tasks, &tasks.pipelines, &tasks.phases, tab)
      .into_iter()
      .filter(|row| filter.includes(&row.organization))
      .map(|row| tasks.decorate_row(row))
      .collect();
    rows.sort_by(|a, b| {
      sort.compare(
        (
          &a.organization,
          tasks.updated.get(&a.id).copied().unwrap_or(a.created_at),
          a.id,
        ),
        (
          &b.organization,
          tasks.updated.get(&b.id).copied().unwrap_or(b.created_at),
          b.id,
        ),
      )
    });
    let table = if rows.is_empty() {
      no_matches(ctx, self.filter.clone())
    } else {
      VirtualizedList::new_shared(ctx, Arc::new(rows))
        .list_key("task-rows")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .mount_keyed::<QueueRow, _, _, _>(|row| row.id, Clone::clone)
    };
    body()
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .child(ctx.mount_keyed::<TaskFilters>(
            "task-filters",
            FilterProps {
              tasks: tasks.clone(),
              filter: self.filter.clone(),
              sort: self.sort.clone(),
            },
          ))
          .child(Spacer::new().flex(1.0))
          .child(
            ActionButton::new(ctx.t("tasks.tags.manageShort"))
              .icon(Icon::Tag)
              .id("project-tags-manage")
              .link(self.nav.link(Page::ProjectTags))
              .render(ctx),
          ),
      )
      .child(ctx.mount_keyed::<SelectionBar>(
        "task-selection",
        SelectionProps {
          root: root.to_owned(),
          tasks: tasks.clone(),
          selected: self.selected.clone(),
        },
      ))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .child(table_heading(ctx))
          .child(table),
      )
      .child(footer(ctx, tasks))
      .into()
  }
}

/// Task and run counts (`Bpth7`).
fn footer(ctx: &Ctx, tasks: &ProjectTasks) -> Element {
  Row::new()
    .id("tasks-footer")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .align_items(Alignment::Center)
    .child(
      Text::new(&message_text(ctx, &list::footer(tasks.tasks.len(), tasks.runs))).variant(TypographyStyle::Caption),
    )
    .into()
}

fn no_matches(ctx: &Ctx, filter: Signal<Filters>) -> Element {
  crate::ui::organization_notice::organization_notice(
    &ctx.t("tasks.filters.noMatches"),
    &ctx.t("tasks.filters.noMatchesHint"),
  )
  .child(
    ActionButton::new(ctx.t("tasks.filters.clear"))
      .on_click(move || filter.set(Filters::default()))
      .render(ctx),
  )
  .into()
}
