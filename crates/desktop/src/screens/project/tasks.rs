//! Tasks (02.03 `J9467`, ORC-39 `qVMJh`): workflow tabs, query and organization filters, select
//! tasks to organize them in bulk or review their first-run scheduling. Shown after New task's
//! Create, the list scrolls to the new task and highlights it.

mod bulk;
mod bulk_cancel;
mod controls;
mod outcome;
mod review;
pub(super) mod selection;

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, VirtualizedList},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::id::{ResourceId, TaskId};

use super::{
  ProjectNav, TasksState, page,
  task_filters::{FilterProps, Filters, TaskFilters},
  task_rows::{QueueRow, QueueRowProps, table_heading},
};
use crate::{
  background,
  messages::Message,
  navigation::route::Page,
  tasks::{
    browse::{self, PipelineFilter, Query, Selection, TaskSort},
    data::ProjectTasks,
    group,
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
const FILTER_HEIGHT: f32 = 36.0;
const PIPELINE_FILTER_WIDTH: f32 = 184.0;
const SORT_WIDTH: f32 = 164.0;
const SELECTION_BUTTON_HEIGHT: f32 = 32.0;
const RUN_SELECTED_WIDTH: f32 = 158.0;
const FOOTER_HEIGHT: f32 = 30.0;

#[derive(Clone, Debug, PartialEq)]
pub struct TasksProps {
  pub project: Arc<str>,
  pub root: Arc<PathBuf>,
  pub runner_online: bool,
  pub agent_slots: Option<std::num::NonZeroU32>,
  pub tab: TaskTab,
  pub tasks: TasksState,
}

impl lurq::app::component::DevtoolsInspectable for TasksProps {}

pub struct TasksScreen {
  nav: ProjectNav,
  selected: Signal<Selection>,
  search: Signal<String>,
  search_focused: Signal<bool>,
  pipeline: Signal<PipelineFilter>,
  /// Status, priority and tag filters (`l7kBSq`).
  filter: Signal<Filters>,
  sort: Signal<TaskSort>,
  reviewing: Signal<bool>,
  assigning: Signal<bool>,
  assign_pipeline: Signal<PipelineFilter>,
  assign: FutureAction<(PathBuf, Vec<TaskId>, ResourceId), Vec<Message>, Message>,
  release: FutureAction<(PathBuf, Vec<TaskId>), (), Message>,
  /// The task Create just made, highlighted while the list is shown.
  created: Option<TaskId>,
}

impl Component for TasksScreen {
  type Props = TasksProps;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let assign = assignment_action(ctx);
    let changed = nav.clone();
    ctx.watch(&assign.state(), move |state| {
      // A failed request keeps the data of the last one that succeeded.
      if state.is_fulfilled() {
        changed.tasks_changed();
      }
    });
    let release = release_action(ctx);
    let changed = nav.clone();
    ctx.watch(&release.state(), move |state| {
      // A failed request keeps the data of the last one that succeeded.
      if state.is_fulfilled() {
        changed.tasks_changed();
      }
    });
    Self {
      created: nav.take_created(),
      nav,
      selected: ctx.signal(Selection::default()),
      search: ctx.signal(String::new()),
      search_focused: ctx.signal(false),
      pipeline: ctx.signal(PipelineFilter::default()),
      filter: ctx.signal(Filters::default()),
      sort: ctx.signal(TaskSort::default()),
      reviewing: ctx.signal(false),
      assigning: ctx.signal(false),
      assign_pipeline: ctx.signal(PipelineFilter::default()),
      assign,
      release,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Keep the create-time actions' polling slots alive on every render.
    let _assign = assignment_action(ctx);
    let _release = release_action(ctx);
    let props = ctx.props::<TasksProps>().clone();
    ctx.provide(selection::TaskSelection(self.selected.clone()));
    if self.reviewing.get() {
      return ctx.mount_keyed::<review::SchedulingReview>(
        "task-scheduling-review",
        review::ReviewProps {
          project: props.project.clone(),
          root: props.root.clone(),
          tasks: props.tasks.clone(),
          selected: self.selected.get().0.iter().copied().collect(),
          close: self.reviewing.clone(),
          runner_online: props.runner_online,
          agent_slots: props.agent_slots,
        },
      );
    }
    let nav = self.nav.clone();
    let tab = props.tab;
    let body = match &props.tasks {
      TasksState::Loading => Column::new().into(),
      TasksState::Failed(error) => load_failed(ctx, &nav, error),
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
      TasksState::Ready(tasks) => library(ctx, self, &props.root, tasks, tab),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(topbar(ctx, &props.project, &nav))
      .child(page::screen_heading(&ctx.t("tasks.title"), &ctx.t("tasks.description")))
      .child(self.tabs(ctx, tab, props.tasks.ready()))
      .child(body)
      .into()
  }
}

fn assignment_action(ctx: &mut Ctx) -> FutureAction<(PathBuf, Vec<TaskId>, ResourceId), Vec<Message>, Message> {
  ctx.future_action(
    |(root, tasks, pipeline): (PathBuf, Vec<TaskId>, ResourceId)| async move {
      background::run(move || group::assign(&root, &tasks, pipeline)).await?
    },
  )
}

fn release_action(ctx: &mut Ctx) -> FutureAction<(PathBuf, Vec<TaskId>), (), Message> {
  ctx.future_action(|(root, tasks): (PathBuf, Vec<TaskId>)| async move {
    background::run(move || group::release(&root, &tasks)).await?
  })
}

impl TasksScreen {
  fn tabs(&self, ctx: &Ctx, selected: TaskTab, tasks: Option<&ProjectTasks>) -> Element {
    let tabs = TaskTab::ALL.into_iter().map(|tab| {
      Tab::new(tab_id(tab), ctx.t(tab.label_key()), Dimension::Px(TAB_ROW_HEIGHT))
        .min_width(Dimension::Px(tab_width(tab)))
        .count(tasks.map(|tasks| list::rows(&tasks.tasks, &tasks.pipelines, &tasks.phases, tab).len()))
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

/// The least width of each workflow tab (`qVMJh`).
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
        .primary()
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

/// The tasks cannot be read: why, and Retry, which reads them again.
fn load_failed(ctx: &Ctx, nav: &ProjectNav, error: &Message) -> Element {
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

/// Filters, the selection bar, the task table and its footer (`npcrO`); the task Create just made
/// is highlighted and scrolled into view once the tasks as read include it.
fn library(ctx: &mut Ctx, screen: &TasksScreen, root: &Path, tasks: &ProjectTasks, tab: TaskTab) -> Element {
  let (search, filter) = (screen.search.get(), screen.filter.get());
  let separator = ctx.t("common.listSeparator");
  let query = Query {
    text: &search,
    pipeline: screen.pipeline.get(),
    organization: &filter.0,
    separator: &separator,
  };
  let rows = Arc::new(browse::rows(tasks, tab, query, screen.sort.get()));
  let visible = rows.clone();
  let created = screen.created;
  let reveal = created
    .filter(|task| rows.iter().any(|row| row.id == *task))
    .map(|task| task.to_string());
  let row_height = super::task_rows::table_row_height(ctx);
  let table: Element = if rows.is_empty() {
    empty_rows(ctx, screen)
  } else {
    VirtualizedList::new_shared(ctx, rows.clone())
      .list_key("task-rows")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .max_height(Dimension::Px(row_height * rows.len() as f32))
      .reveal_key(reveal)
      .mount_keyed::<QueueRow, _, _, _>(
        |row| row.id,
        move |row| QueueRowProps {
          row: row.clone(),
          created: created == Some(row.id),
        },
      )
  };
  let content = body()
    .child(controls::filters(ctx, screen, tasks))
    .child(organization_filters(ctx, screen, tasks))
    .with_children(controls::selection_bar(ctx, screen, root, tasks, visible.clone()))
    .with_children(
      screen
        .assigning
        .get()
        .then(|| controls::assignment(ctx, screen, root, tasks)),
    )
    .with_children(controls::problems(ctx, screen, tasks))
    .child(table_column(ctx, table, visible.len()))
    .child(footer(ctx, tasks, &visible, filtered(screen)));
  crate::ui::form::form(None, content)
}

/// Status, priority and tag filters with what they hold, and Manage tags (`qVMJh`, `l7kBSq`).
fn organization_filters(ctx: &mut Ctx, screen: &TasksScreen, tasks: &ProjectTasks) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(ctx.mount_keyed::<TaskFilters>(
      "task-filters",
      FilterProps {
        tasks: tasks.clone(),
        filter: screen.filter.clone(),
      },
    ))
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new(ctx.t("tasks.tags.manageShort"))
        .icon(Icon::Tags)
        .id("project-tags-manage")
        .link(screen.nav.link(Page::ProjectTags))
        .render(ctx),
    )
    .into()
}

/// The table under its heading: as tall as its `rows` up to the room left, so the footer follows
/// the last row; without rows, as tall as what it says instead.
fn table_column(ctx: &Ctx, table: Element, rows: usize) -> Element {
  let column = Column::new()
    .width(Dimension::Pct(100.0))
    .child(table_heading(ctx))
    .child(table);
  if rows == 0 {
    return column.into();
  }
  column
    .flex(1.0)
    .max_height(Dimension::Px(
      super::task_rows::HEADING_HEIGHT + super::task_rows::table_row_height(ctx) * rows as f32,
    ))
    .into()
}

/// No row to show: the tab has no tasks, or the search, pipeline or organization filters hide
/// them all, which the notice says and the person can clear (`rVJoA`).
fn empty_rows(ctx: &Ctx, screen: &TasksScreen) -> Element {
  let filtered = filtered(screen);
  let (search, pipeline, filter) = (screen.search.clone(), screen.pipeline.clone(), screen.filter.clone());
  let message: Element = if filtered {
    crate::ui::organization_notice::organization_notice(
      &ctx.t("tasks.emptyQuery.title"),
      &ctx.t("tasks.emptyQuery.detail"),
    )
    .into()
  } else {
    Text::new(&ctx.t("tasks.emptyTab"))
      .variant(TypographyStyle::Caption)
      .into()
  };
  Column::new()
    .id("tasks-empty-rows")
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(message)
    .with_children(filtered.then(|| {
      ActionButton::new(ctx.t("tasks.filters.clear"))
        .icon(Icon::X)
        .id("tasks-clear-query")
        .in_form()
        .on_click(move || {
          search.set(String::new());
          pipeline.set(PipelineFilter::default());
          filter.set(Filters::default());
        })
        .render(ctx)
    }))
    .into()
}

/// Whether the search, the pipeline or an organization filter narrows the rows of the tab.
fn filtered(screen: &TasksScreen) -> bool {
  !screen.search.get().trim().is_empty()
    || screen.pipeline.get().0.is_some()
    || screen.filter.get() != Filters::default()
}

/// The counts of the rows shown and their runs (`Bpth7`), or how many match the query (`l7kBSq`).
fn footer(ctx: &Ctx, tasks: &ProjectTasks, rows: &[list::TaskRow], filtered: bool) -> Element {
  let count = list::footer(rows.len(), tasks.runs_of(rows), filtered);
  Row::new()
    .id("tasks-footer")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .align_items(Alignment::Center)
    .child(Text::new(&message_text(ctx, &count)).variant(TypographyStyle::Caption))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t("tasks.editingHint")).variant(TypographyStyle::Caption))
    .into()
}
