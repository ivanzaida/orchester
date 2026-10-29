//! The project's Tasks list (`Tasks · redesign` `Lw1Gb` in the new `orchester.pen`, ORC-99): one header row with the
//! search, List or Board, Filter, Display, *More actions* and New task; the views Needs you (ORC-105), Active, Backlog,
//! Done and All with their counts; the applied filters as chips; and the view's rows, one line each, Active grouped by
//! what happens next (Needs you, Running, Queued, Ready to start, then Stopped and Finished, which the design has no
//! row for) and All by workflow status (`views::grouped`). Each row says in one phrase where its task stands and opens
//! the task; under the pointer it shows its actions. The task a Create just added stands out. Its other states: a
//! project without tasks (`03 No tasks yet` `f54MO`), the tasks being read (a skeleton, `skeleton`), read failing, and
//! a search or filters that hide every row. What waits for the person and is no task's (the project's memory records)
//! is named in a notice over Needs you's and Active's rows, which opens the Needs you panel.
//!
//! The same header, views, filters and notices stand over the board (ORC-105, `kanban`), which shows the view's tasks
//! as cards in columns; List or Board is kept for the project (`kanban::display`).
//!
//! The tasks are the project feed's, as they stand with its runner: while no runner serves the project, a run recorded
//! as running is interrupted. Not built yet (D4): selecting the list's rows, Manage tags.

mod board_view;
mod cache;
mod display;
mod facets;
mod header;
mod kanban;
mod list;
mod not_shared;
mod phrase;
mod queue;

pub(crate) use self::{list::age, phrase::phrases, queue::actor_now};
mod row;
mod shared;
mod skeleton;
mod states;
mod views;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::project::TaskKey;
use orchester_workspace::{
  runner::{RunnerStatus, state::RunnerState},
  tasks::{
    browse::{self, Facets, TaskSort},
    data::ProjectTasks,
    views::grouped,
  },
};
use parking_lot::Mutex;

use self::{
  board_view::{keep_display, kept_display},
  cache::{ListCache, Reading, Rows, RowsKey},
  facets::Options,
  header::{Choices, Header},
  kanban::{
    display::{Display, Layout},
    has_empty_columns,
  },
  list::{Folded, Folding, Source},
  row::ItemProps,
  views::{Counts, label_key, list_view},
};
use crate::{
  navigation::Navigation,
  project::{ShownProject, TasksState},
  routes::{TaskList, TaskListTab},
  screens::task_close::{self, Closer},
  theme::TasksSpacing,
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
  /// The groups folded away.
  folded: Signal<Folded>,
  /// `/` asked for the search: the next render focuses it.
  focus_search: Signal<bool>,
  /// A hold on new starts is being changed ([`queue::Changing`]).
  changing: Signal<bool>,
  cache: ListCache,
  /// Where the rows are scrolled to, and what they were last placed for ([`TasksScreen::place`]).
  scroll: ScrollState,
  placed: Mutex<Option<Placed>>,
  /// Closing, reopening and deleting tasks from their rows' menus, asked in place over the rows.
  closer: Closer,
  /// List or Board, and the board's options, kept for the project.
  display: Signal<Display>,
}

impl Component for TasksScreen {
  type Props = TasksProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<TasksProps>().clone();
    let typed = props.list.search().unwrap_or_default().to_owned();
    let search = ctx.signal(typed);
    let navigation = Navigation::of(ctx);
    ctx.watch(&search, move |text: &String| header::follow_search(&navigation, text));
    let shown = kept_display(ctx, &props.project);
    let display = ctx.signal(shown);
    keep_display(ctx, &props.project, &display);
    Self {
      choices: Choices {
        search,
        facets: ctx.signal(Facets::default()),
        sort: ctx.signal(TaskSort::default()),
        search_input: ElementRef::default(),
      },
      folded: ctx.signal(Folded::default()),
      focus_search: ctx.signal(false),
      changing: ctx.signal(false),
      cache: ListCache::default(),
      scroll: ScrollState::new(),
      placed: Mutex::new(None),
      closer: Closer::new(ctx, (props.project.open, props.project.feed), None),
      display,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    ctx.provide(self.closer.clone());
    ctx.provide(Folding(self.folded.clone()));
    ctx.provide(queue::Changing(self.changing.clone()));
    let TasksProps { project, list } = ctx.props::<TasksProps>().clone();
    // The path's search wins: Back, Clear and automation change it too.
    let routed = list.search().unwrap_or_default();
    if self.choices.search.get_untracked() != routed {
      self.choices.search.set(routed.to_owned());
    }
    if self.focus_search.get() {
      ctx.focus(&self.choices.search_input);
      self.focus_search.set(false);
    }
    let id = project.open.project.id();
    let shared = shared::read(ctx, &project);
    let not_member = shared
      .as_ref()
      .and_then(|shared| shared::not_member(ctx, &project, shared));
    // The page keeps 28 at its left; its parts keep the 28 at their right themselves ([`inset`]), so the list can run
    // to the panel's edge with its scrollbar there.
    let page = Column::new()
      .id("screen-tasks")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Xl).left(TasksSpacing::Sides.role()))
      .spacing(TasksSpacing::Stack.role())
      .on_key_down(slash(self.focus_search.clone()));
    let header = |ctx: &mut Ctx, options, queue| {
      inset(header::header(
        ctx,
        Header {
          project: id,
          choices: &self.choices,
          options,
          queue,
          shared: shared.as_ref(),
          display: &self.display,
        },
      ))
    };
    let not_member = not_member.map(inset);
    match project.feed.tasks.get() {
      TasksState::Ready(tasks) if tasks.tasks.is_empty() && tasks.not_shared.is_empty() => {
        let read_only = shared.as_ref().and_then(|shared| shared.read_only(ctx));
        page
          .child(inset(header::title_only(ctx, id, shared.as_ref())))
          .with_children(not_member)
          .child(inset(states::empty_project(ctx, id, read_only)).flex(1.0))
      }
      TasksState::Ready(source) => {
        // The filter is gone once no task is Not shared: the list is the shared one again.
        let not_shared = list.not_shared() && !source.not_shared.is_empty();
        let shown = Shown {
          project: &project,
          list: &list,
          source: &source,
          shared: shared.as_ref(),
          not_shared,
        };
        let options = Arc::new(Options::of(ctx, &source));
        let chosen = self.choices.facets.get_untracked();
        let offered = options.offered(&chosen);
        if offered.count() != chosen.count() {
          self.choices.facets.set(offered);
        }
        let page = page.child(header(ctx, Some(options.clone()), queue(&project)));
        self.list(ctx, page.with_children(not_member), &shown, &options)
      }
      TasksState::Loading => {
        let page = page.child(header(ctx, None, None)).with_children(not_member);
        self.reading(ctx, page, (&project, &list))
      }
      TasksState::Failed(error) => page
        .child(header(ctx, None, None))
        .with_children(not_member)
        .child(inset(views::views(ctx, (id, &list), Counts::Unknown, None)))
        .child(inset(states::failed(ctx, &project.feed, &error))),
    }
  }
}

impl TasksScreen {
  /// While the tasks are read: the views counting with bones, and the skeleton of the list or the board.
  fn reading(&self, ctx: &mut Ctx, page: Column, (project, list): (&ShownProject, &TaskList)) -> Column {
    let id = project.open.project.id();
    let display = self.display.get();
    let place = match display.layout {
      Layout::List => skeleton::Place::List,
      Layout::Board => skeleton::Place::Board(list.tab(), display.columns),
    };
    let label = ctx.t_args("tasks.loading", [("project", project.open.project.name().as_str())]);
    let views = inset(views::views(ctx, (id, list), Counts::Reading, None));
    page.child(views).child(skeleton::skeleton(ctx, place, label))
  }
  /// The views, what is held and filtered, and the view's rows as searched, filtered and ordered, or why there is none.
  fn list(&self, ctx: &mut Ctx, page: Column, shown: &Shown<'_>, options: &Options) -> Column {
    let (project, list) = (shown.project, shown.list);
    let id = project.open.project.id();
    let separator = ctx.t("tasks.listSeparator");
    let (reading, totals) = self
      .cache
      .reading(shown.source, (online(project), shown.not_shared), |tasks| {
        browse::faceted(tasks, ("", &separator), &Facets::default(), TaskSort::Manual)
      });
    let key = RowsKey {
      not_shared: shown.not_shared,
      search: list.search().unwrap_or_default().to_owned(),
      facets: self.choices.facets.get(),
      sort: self.choices.sort.get(),
    };
    let Rows { rows, counts } = self.cache.rows(key.clone(), |tasks| {
      browse::faceted(tasks, (&key.search, &separator), &key.facets, key.sort)
    });
    let page = self.top(ctx, page, shown, (&reading, counts), options);
    let parts = grouped(
      list_view(list.tab()),
      &rows,
      (&reading.tasks, &reading.waiting),
      key.sort,
    );
    let filtered = !key.search.trim().is_empty() || !key.facets.is_empty();
    if parts.is_empty() {
      // Rows that come back are placed afresh, at their top.
      *self.placed.lock() = None;
      let found = if shown.not_shared {
        Vec::new()
      } else {
        not_shared::matching(shown.source, &key.search, &separator)
      };
      if !found.is_empty() {
        return page.child(inset(not_shared::only_not_shared(ctx, (id, list), &found)));
      }
      // A view the search and filters empty says so in the columns' place; an empty view shows its empty columns, and
      // says so too where it has none (Needs you, columns by pipeline).
      let display = self.display.get();
      if display.layout == Layout::List || filtered || !has_empty_columns(list.tab(), &display.columns) {
        return page.child(inset(self.no_rows(ctx, list.tab(), totals.get(list.tab()), filtered)));
      }
    }
    let list_key = format!("tasks-rows-{:?}-{key:?}", list.tab());
    if self.display.get().layout == Layout::Board {
      return page.child(self.board(ctx, shown, (&reading, &rows), (filtered, list_key)));
    }
    let highlight = list.highlight().map(ToString::to_string);
    let source = Source {
      project: id,
      tasks: &reading.tasks,
      needs: &reading.needs,
      highlight: highlight.as_deref(),
      shared: shown.shared.is_some(),
    };
    let items = list::items(ctx, &source, &parts, &self.folded.get());
    // A new list for what narrows and orders the rows, and once a highlighted task the rows lacked comes: a list keeps
    // its top row in place while its rows change, which these must not.
    let list_key = match self.place((list.tab(), key), list.highlight(), &items) {
      true => list_key,
      false => format!("{list_key}-awaiting"),
    };
    page.child(list::list(ctx, Arc::new(items), (self.scroll.clone(), list_key)))
  }

  /// The views with the held chip at their end, the held groups, the applied filters, the *Not shared* notice, what
  /// waits outside the tasks, and the close confirmation's panel.
  fn top(
    &self,
    ctx: &mut Ctx,
    page: Column,
    shown: &Shown<'_>,
    (reading, counts): (&Reading, cache::ViewCounts),
    options: &Options,
  ) -> Column {
    let (project, list) = (shown.project, shown.list);
    let id = project.open.project.id();
    let chip = queue_part(ctx, project, queue::QueuePart::Chip);
    let held = (!reading.tasks.queue.groups.is_empty()).then(|| queue_part(ctx, project, queue::QueuePart::Groups));
    let mut filters: Vec<Element> = not_shared::filter(ctx, (id, list), shown.source.not_shared.len())
      .into_iter()
      .collect();
    filters.extend(facets::chips(ctx, &self.choices.facets, options));
    let filters = (!filters.is_empty()).then(|| {
      Row::new()
        .id("tasks-filters")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(filters)
    });
    let kept = shown.not_shared.then(|| {
      let name = project.open.project.name().as_str();
      not_shared::notice(ctx, (&shown.source.not_shared, name), shown.shared)
    });
    // The board asks at the card, in a popover over the columns (`Popover · stop run` `XdfNz`): only why the last
    // close failed stands over them.
    let close = if self.display.get().layout == Layout::Board {
      task_close::failure(ctx, &self.closer, "tasks-close")
    } else {
      task_close::panel(ctx, &self.closer, ("tasks-close", task_close::StopLook::Quiet))
    };
    page
      .child(inset(views::views(ctx, (id, list), Counts::Read(counts), chip)))
      .with_children(held.flatten().map(inset))
      .with_children(filters.map(inset))
      .with_children(kept.map(inset))
      .with_children(states::outside_tasks(ctx, list.tab(), &reading.tasks).map(inset))
      .with_children(close.map(inset))
  }

  /// Scrolls the rows when what they show changed: to the task a Create just added, or else to the top, for another
  /// view, search, filter or sort. A highlighted task the rows do not hold yet (the feed is still reading the tasks
  /// again) is scrolled to once a reading brings it. A render for anything else leaves the rows where they are.
  /// Answers whether the rows hold the highlighted task, if any.
  fn place(&self, key: (TaskListTab, RowsKey), highlight: Option<&TaskKey>, items: &[ItemProps]) -> bool {
    let mut placed = self.placed.lock();
    let same = placed
      .as_ref()
      .is_some_and(|placed| placed.key == key && placed.highlight.as_ref() == highlight);
    if same && placed.as_ref().is_some_and(|placed| placed.reached) {
      return true;
    }
    let found = highlight.and_then(|task| list::offset_of(items, &task.to_string()));
    match found {
      Some(offset) => self.scroll.set_scroll_pending(0.0, offset),
      // Still waiting for the highlighted task: the rows stay where they were put.
      None if same => return false,
      // An offset, not a scroll to the top: a list mounted anew opens at a queued offset, or else at the one the
      // shared scroll state kept from the rows before.
      None => self.scroll.set_scroll_pending(0.0, 0.0),
    }
    let reached = highlight.is_none() || found.is_some();
    *placed = Some(Placed {
      key,
      highlight: highlight.cloned(),
      reached,
    });
    reached
  }

  /// Why `view` shows no row: it has none of its `count` tasks at all, or the search and the filters hide them, which
  /// Clear undoes.
  fn no_rows(&self, ctx: &Ctx, view: TaskListTab, count: usize, filtered: bool) -> Column {
    if count == 0 || !filtered {
      return states::empty_view(ctx, view);
    }
    let (search, facets) = (self.choices.search.clone(), self.choices.facets.clone());
    states::no_matches(ctx, (&ctx.t(label_key(view)), count), move || {
      search.set(String::new());
      facets.set(Facets::default());
    })
  }
}

/// `part` 28 from the panel's right edge, where the page leaves that room to the list's scrollbar.
fn inset(part: impl Into<Element>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().right(TasksSpacing::Sides.role()))
    .child(part)
}

/// `/` asks for the search, unless a text field has the keys or a modifier is held.
fn slash(focus: Signal<bool>) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  move |event: KeyboardEvent| {
    if event.default_prevented() || event.text_input_focused || event.ctrl || event.alt || event.meta {
      return;
    }
    if event.key == "/" {
      event.prevent_default();
      focus.set(true);
    }
  }
}

/// `part` of the holds on new starts, once the tasks are read.
fn queue_part(ctx: &mut Ctx, project: &ShownProject, part: queue::QueuePart) -> Option<Element> {
  let props = queue(project)?;
  let name = match part {
    queue::QueuePart::Menu { .. } => "menu",
    queue::QueuePart::Chip => "chip",
    queue::QueuePart::Groups => "groups",
  };
  let key = format!("tasks-queue-{name}-{}", project.open.project.id());
  Some(ctx.mount_keyed::<queue::QueueControls>(&key, queue::QueueProps { part, ..props }))
}

/// The holds on new starts, once the tasks are read; the mount picks its part.
fn queue(project: &ShownProject) -> Option<queue::QueueProps> {
  match project.feed.tasks.get() {
    TasksState::Ready(tasks) => Some(queue::QueueProps {
      project: project.open.project.id(),
      root: project.open.repo_root.clone(),
      queue: (*tasks.queue).clone(),
      part: queue::QueuePart::Chip,
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

/// What the list is drawn from: the project, its route, the feed's tasks, its sharing, and whether the *Not shared*
/// filter is on.
struct Shown<'a> {
  project: &'a ShownProject,
  list: &'a TaskList,
  source: &'a ProjectTasks,
  shared: Option<&'a shared::Shared>,
  not_shared: bool,
}

/// What the rows were last placed for, and whether the highlighted task, if any, was among them.
struct Placed {
  key: (TaskListTab, RowsKey),
  highlight: Option<TaskKey>,
  reached: bool,
}
