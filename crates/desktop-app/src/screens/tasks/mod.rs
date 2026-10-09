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
mod view;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::Column,
  layout::layout_kind::ScrollState,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{project::TaskKey, task::organization::TaskFilter};
use orchester_workspace::{
  runner::{RunnerStatus, state::RunnerState},
  tasks::{
    browse::{self, PipelineFilter, Query, TaskSort},
    data::ProjectTasks,
    list::TaskRow,
  },
};
use parking_lot::Mutex;

use self::{
  query::Choices,
  table::Rows,
  view::{ListView, RowsKey, TabCounts},
};
use crate::{
  navigation::Navigation,
  project::{ShownProject, TasksState},
  routes::{TaskList, TaskListTab},
  theme::{ExtraSpacing, TasksSpacing},
  ui::table::ROW_HEIGHT,
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
  view: ListView,
  /// Where the rows are scrolled to, and what they were last placed for ([`TasksScreen::place`]).
  scroll: ScrollState,
  placed: Mutex<Option<(RowsKey, Option<TaskKey>)>>,
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
      view: ListView::default(),
      scroll: ScrollState::new(),
      placed: Mutex::new(None),
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
    match project.feed.tasks.get() {
      TasksState::Ready(tasks) if tasks.tasks.is_empty() => page.child(states::empty_project(ctx, id)),
      TasksState::Ready(tasks) => {
        let (tasks, counts) = self.view.tasks(&tasks, online(&project));
        self.list(ctx, page, (&project, &list), (&tasks, counts))
      }
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
    (tasks, counts): (&ProjectTasks, TabCounts),
  ) -> Column {
    let id = project.open.project.id();
    let key = RowsKey {
      tab: list.tab(),
      search: list.search().unwrap_or_default().to_owned(),
      pipeline: self.choices.pipeline.get(),
      sort: self.choices.sort.get(),
    };
    let filtered = !key.search.trim().is_empty() || key.pipeline.0.is_some();
    let separator = ctx.t("tasks.listSeparator");
    let rows = self.view.rows(key.clone(), |tasks| {
      let organization = TaskFilter::default();
      let query = Query {
        text: &key.search,
        pipeline: key.pipeline,
        organization: &organization,
        separator: &separator,
      };
      browse::rows(tasks, header::workspace_tab(key.tab), query, key.sort)
    });
    let page = page
      .child(header::tabs(ctx, id, list, Some(counts)))
      .child(query::query(ctx, &self.choices, Some(tasks)));
    if rows.is_empty() {
      return page.child(self.no_rows(ctx, list.tab(), counts.get(list.tab()), filtered));
    }
    let list_key = format!("tasks-rows-{key:?}");
    self.place(key, list.highlight(), &rows);
    let highlight = list.highlight().map(ToString::to_string);
    let shown = Rows {
      project: id,
      tasks,
      rows,
      highlight: highlight.as_deref(),
      scroll: self.scroll.clone(),
      list_key,
    };
    page
      .child(table::table(ctx, &shown))
      .child(table::footer(ctx, &shown, filtered))
  }

  /// Scrolls the rows when what they show changed: to the task a Create just added, or else to the top,
  /// for another tab, search, pipeline or sort. A render for anything else leaves them where they are.
  fn place(&self, key: RowsKey, highlight: Option<&TaskKey>, rows: &[TaskRow]) {
    let placing = (key, highlight.cloned());
    let mut placed = self.placed.lock();
    if placed.as_ref() == Some(&placing) {
      return;
    }
    let shown = highlight.and_then(|task| {
      let task = task.to_string();
      rows.iter().position(|row| *row.key == *task)
    });
    match shown {
      Some(index) => self.scroll.set_scroll_pending(0.0, ROW_HEIGHT * index as f32),
      None => self.scroll.scroll_to_top_pending(),
    }
    *placed = Some(placing);
  }

  /// Why `tab` shows no row: it has none of its `count` tasks at all, or the query hides them, which
  /// Clear undoes.
  fn no_rows(&self, ctx: &Ctx, tab: TaskListTab, count: usize, filtered: bool) -> Column {
    let word = ctx.t(header::label_key(tab));
    if count == 0 || !filtered {
      return states::empty_tab(ctx, &word, tab == TaskListTab::NeedsYou);
    }
    let (search, pipeline) = (self.choices.search.clone(), self.choices.pipeline.clone());
    states::no_matches(ctx, (&word, count), move || {
      search.set(String::new());
      pipeline.set(PipelineFilter::default());
    })
  }
}

/// Whether the project's runner serves it: without one, a run recorded as running is interrupted.
fn online(project: &ShownProject) -> bool {
  let root = &project.open.repo_root;
  project
    .feed
    .status
    .get()
    .as_ref()
    .map(|status: &RunnerStatus| status.state_for(root))
    == Some(RunnerState::Online)
}
