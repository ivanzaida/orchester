//! The project's Tasks list (`Tasks · full page · dark` `C2E5Po`): its tasks by workflow tab, searched and
//! narrowed to a pipeline, in the order chosen, each row saying what the task needs or where its run
//! stands, and opening the task. The task a Create just added stands out (`Tasks · new task highlighted`
//! `t3oAOi`). Its other states: a project without tasks (`PMJrB`), the tasks being read (`kA8bx`), read
//! failing (`oDyZB`) and a search that hides every row (`EHGLs`).
//!
//! The tasks are the project feed's, as they stand with its runner: while no runner serves the project, a
//! run recorded as running is interrupted. Not built yet (D4): selecting tasks and what is done to a
//! selection, the Status, Priority and Tags filters and Manage tags.

mod header;
mod query;
mod readiness;
mod row;
mod states;
mod table;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::Column,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::task::organization::TaskFilter;
use orchester_workspace::{
  runner::{RunnerStatus, state::RunnerState},
  tasks::{
    browse::{self, PipelineFilter, Query, TaskSort},
    data::ProjectTasks,
  },
};

use self::{query::Choices, table::Rows};
use crate::{
  navigation::Navigation,
  project::{ShownProject, TasksState},
  routes::{TaskList, TaskListTab},
  theme::{ExtraSpacing, TasksSpacing},
};

pub fn screen(ctx: &mut Ctx, project: &ShownProject, list: &TaskList) -> Element {
  let key = format!("tasks-{}", project.open.project.id());
  ctx.mount_keyed::<TasksScreen>(
    &key,
    TasksProps {
      project: project.clone(),
      list: list.clone(),
    },
  )
}

#[derive(Clone, PartialEq)]
struct TasksProps {
  project: ShownProject,
  list: TaskList,
}

impl DevtoolsInspectable for TasksProps {}

struct TasksScreen {
  choices: Choices,
}

impl Component for TasksScreen {
  type Props = TasksProps;

  fn create(ctx: &mut Ctx) -> Self {
    let typed = ctx.props::<TasksProps>().list.search().unwrap_or_default().to_owned();
    let search = ctx.signal(typed);
    let navigation = Navigation::of(ctx);
    ctx.watch(&search, move |text: &String| query::follow_search(&navigation, text));
    Self {
      choices: Choices {
        search,
        pipeline: ctx.signal(PipelineFilter::default()),
        sort: ctx.signal(TaskSort::default()),
      },
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let TasksProps { project, list } = ctx.props::<TasksProps>().clone();
    // The path's search wins: Back, Clear and automation change it too.
    let routed = list.search().unwrap_or_default();
    if self.choices.search.get_untracked() != routed {
      self.choices.search.set(routed.to_owned());
    }
    let id = project.open.project.id();
    let page = Column::new()
      .id("screen-tasks")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Roomy.role())
          .left(TasksSpacing::Page.role())
          .right(TasksSpacing::Page.role()),
      )
      .spacing(SpacingSize::Md)
      .child(header::header(ctx, id));
    match shown_tasks(&project) {
      TasksState::Ready(tasks) if tasks.tasks.is_empty() => page.child(states::empty_project(ctx, id)),
      TasksState::Ready(tasks) => self.list(ctx, page, (&project, &list), &tasks),
      TasksState::Loading => page
        .child(header::tabs(ctx, id, &list, None))
        .child(query::query(ctx, &self.choices, None))
        .child(states::reading(ctx, project.open.project.name().as_str())),
      TasksState::Failed(error) => page
        .child(header::tabs(ctx, id, &list, None))
        .child(query::out_of_reach(query::query(ctx, &self.choices, None)))
        .child(states::failed(ctx, &project.feed, &error)),
    }
  }
}

impl TasksScreen {
  /// The tabs, the query and the tab's rows as searched, narrowed and ordered, with their count, or why
  /// there is none.
  fn list(
    &self,
    ctx: &mut Ctx,
    page: Column,
    (project, list): (&ShownProject, &TaskList),
    tasks: &ProjectTasks,
  ) -> Column {
    let id = project.open.project.id();
    let tab = header::workspace_tab(list.tab());
    let pipeline = self.choices.pipeline.get();
    let search = list.search().unwrap_or_default();
    let separator = ctx.t("tasks.listSeparator");
    let organization = TaskFilter::default();
    let query = Query {
      text: search,
      pipeline,
      organization: &organization,
      separator: &separator,
    };
    let rows = Arc::new(browse::rows(tasks, tab, query, self.choices.sort.get()));
    let filtered = !search.trim().is_empty() || pipeline.0.is_some();
    let page =
      page
        .child(header::tabs(ctx, id, list, Some(tasks)))
        .child(query::query(ctx, &self.choices, Some(tasks)));
    if rows.is_empty() {
      return page.child(self.no_rows(ctx, list.tab(), tasks, filtered));
    }
    let highlight = list.highlight().map(ToString::to_string);
    let shown = Rows {
      project: id,
      tasks,
      rows,
      highlight: highlight.as_deref(),
    };
    page
      .child(table::table(ctx, &shown))
      .child(table::footer(ctx, &shown, filtered))
  }

  /// Why `tab` shows no row: the query hides its tasks, which Clear undoes, or it has none.
  fn no_rows(&self, ctx: &Ctx, tab: TaskListTab, tasks: &ProjectTasks, filtered: bool) -> Column {
    let word = ctx.t(header::label_key(tab));
    if !filtered {
      return states::empty_tab(ctx, &word, tab == TaskListTab::NeedsYou);
    }
    let count = header::count(tasks, tab);
    let (search, pipeline) = (self.choices.search.clone(), self.choices.pipeline.clone());
    states::no_matches(ctx, (&word, count), move || {
      search.set(String::new());
      pipeline.set(PipelineFilter::default());
    })
  }
}

/// The feed's tasks as they stand with the project's runner: without one serving it, a run recorded as
/// running is interrupted.
fn shown_tasks(project: &ShownProject) -> TasksState {
  let root = &project.open.repo_root;
  let online = project
    .feed
    .status
    .get()
    .as_ref()
    .map(|status: &RunnerStatus| status.state_for(root))
    == Some(RunnerState::Online);
  match project.feed.tasks.get() {
    TasksState::Ready(tasks) if !online => TasksState::Ready(tasks.without_runner()),
    state => state,
  }
}
