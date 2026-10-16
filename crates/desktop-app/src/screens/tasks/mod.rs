//! The project's Tasks list (`Tasks · full page · dark` `C2E5Po`): its tasks by workflow tab, searched and
//! narrowed to a pipeline, in the order chosen, each row saying what the task needs or where its run
//! stands, and opening the task. The task a Create just added stands out (`Tasks · new task highlighted`
//! `t3oAOi`). Its other states: a project without tasks (`PMJrB`), the tasks being read (`kA8bx`), read
//! failing (`oDyZB`) and a search that hides every row (`EHGLs`). Needs you lists and counts the tasks
//! something of waits for; what waits and is no task's (the project's memory records) is named in a notice
//! over its rows, which opens the Needs you panel.
//!
//! The tasks are the project feed's, as they stand with its runner: while no runner serves the project, a
//! run recorded as running is interrupted. Not built yet (D4): selecting tasks and what is done to a
//! selection, the Status, Priority and Tags filters and Manage tags.

mod header;
mod query;
mod queue;

pub(crate) use self::queue::actor_now;
mod readiness;
mod row;
mod shared;
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
  placed: Mutex<Option<Placed>>,
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
    let shared = shared::read(ctx, &project);
    let not_member = shared
      .as_ref()
      .and_then(|shared| shared::not_member(ctx, &project, shared));
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
      .child(header::header(ctx, id, queue(&project), shared.as_ref()))
      .with_children(not_member);
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
      .with_children(held_groups(ctx, project, tasks))
      .child(query::query(ctx, &self.choices, Some(tasks)))
      .with_children(states::outside_tasks(ctx, list.tab(), tasks));
    if rows.is_empty() {
      // Rows that come back are placed afresh, at their top.
      *self.placed.lock() = None;
      return page.child(self.no_rows(ctx, list.tab(), counts.get(list.tab()), filtered));
    }
    // A new list for what narrows and orders the rows, and once a highlighted task the rows lacked comes:
    // a list keeps its top row in place while its rows change, which these must not.
    let list_key = format!("tasks-rows-{key:?}");
    let list_key = match self.place(key, list.highlight(), &rows) {
      true => list_key,
      false => format!("{list_key}-awaiting"),
    };
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
  /// for another tab, search, pipeline or sort. A highlighted task the rows do not hold yet (the feed is
  /// still reading the tasks again) is scrolled to once a reading brings it. A render for anything else
  /// leaves the rows where they are. Answers whether the rows hold the highlighted task, if any.
  fn place(&self, key: RowsKey, highlight: Option<&TaskKey>, rows: &[TaskRow]) -> bool {
    let mut placed = self.placed.lock();
    let same = placed
      .as_ref()
      .is_some_and(|placed| placed.key == key && placed.highlight.as_ref() == highlight);
    if same && placed.as_ref().is_some_and(|placed| placed.reached) {
      return true;
    }
    let found = highlight.and_then(|task| {
      let task = task.to_string();
      rows.iter().position(|row| *row.key == *task)
    });
    match found {
      Some(index) => self.scroll.set_scroll_pending(0.0, ROW_HEIGHT * index as f32),
      // Still waiting for the highlighted task: the rows stay where they were put.
      None if same => return false,
      None => self.scroll.scroll_to_top_pending(),
    }
    let reached = highlight.is_none() || found.is_some();
    *placed = Some(Placed {
      key,
      highlight: highlight.cloned(),
      reached,
    });
    reached
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

/// The groups whose new starts are held, under the tabs; nothing when none is.
fn held_groups(ctx: &mut Ctx, project: &ShownProject, tasks: &ProjectTasks) -> Option<Element> {
  let id = project.open.project.id();
  (!tasks.queue.groups.is_empty()).then(|| {
    ctx.mount_keyed::<queue::QueueControls>(
      &format!("tasks-queue-groups-{id}"),
      queue::QueueProps {
        project: id,
        root: project.open.repo_root.clone(),
        queue: (*tasks.queue).clone(),
        part: queue::QueuePart::Groups,
      },
    )
  })
}

/// The holds on new starts, once the tasks are read.
fn queue(project: &ShownProject) -> Option<queue::QueueProps> {
  match project.feed.tasks.get() {
    TasksState::Ready(tasks) => Some(queue::QueueProps {
      project: project.open.project.id(),
      root: project.open.repo_root.clone(),
      queue: (*tasks.queue).clone(),
      part: queue::QueuePart::Project,
    }),
    TasksState::Loading | TasksState::Failed(_) => None,
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

/// What the rows were last placed for, and whether the highlighted task, if any, was among them.
struct Placed {
  key: RowsKey,
  highlight: Option<TaskKey>,
  reached: bool,
}
