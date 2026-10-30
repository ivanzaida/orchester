//! A task's Memory tab (`Task · Memory · dark` `a0PLO` in `g5dmWI`): the toolbar with the search, the *Type*,
//! *Status* and *Author* filters, the count and *New record*; the task's records grouped by type with the proposals
//! waiting for approval first, each changed in place (`hETrL` reviewing a proposal, `i4zIrS` editing a record,
//! `VFZl3` deleting one); and *Next agent receives* at the right. With no record yet it says what agents keep
//! (`e8oWP4`); a failed reading says why with *Try again* (`AlQy0`). The count of its active records is the tab's.

mod next;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::TaskId;
use orchester_workspace::{
  background,
  memory::{
    listing::{self, Filters, StatusFilter},
    read::{TaskMemory, active_count_in},
  },
  messages::{self, Message},
};

use super::TaskContext;
use crate::{
  screens::memory::{
    self as memory_ui, Centered, FilterBar, FiltersProps, ListInput,
    state::{Made, MemoryState, OfTask, Opened, Shared},
    words,
  },
  services::Services,
  theme::{ExtraTypography, MemorySpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    skeleton::{self, Skeleton},
    state::EmptyState,
  },
};

/// The toolbar's height (`yFMam`).
const TOOLBAR_HEIGHT: f32 = 46.0;

#[derive(Clone, PartialEq)]
pub struct MemoryProps {
  pub task: TaskContext,
}

impl DevtoolsInspectable for MemoryProps {}

/// The tab's mount point.
pub fn tab(ctx: &mut Ctx, task: &TaskContext) -> Element {
  ctx.mount_keyed::<TaskMemoryTab>("task-memory", MemoryProps { task: task.clone() })
}

pub struct TaskMemoryTab {
  made: Made,
  retries: Signal<u64>,
}

impl Component for TaskMemoryTab {
  type Props = MemoryProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<MemoryProps>().clone();
    Self {
      made: Made::new(ctx, Filters::default(), &props.task.project.feed),
      retries: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<MemoryProps>().clone();
    let task = OfTask {
      id: props.task.detail.task.id(),
      key: props.task.detail.task.key().clone(),
    };
    let state = self.made.state(ctx, &props.task.project, Some(task));
    ctx.provide(state.clone());
    let read = read(ctx, &props.task, self.retries.get());
    let filters = state.filters.get();
    // The search and filters show once there are records to search; the count once they are read. With no record
    // yet the empty state has *New record* itself, so the toolbar is left out (`e8oWP4`).
    let (searchable, counted) = match &read {
      Read::Ready(memory) => (!memory.records.is_empty(), Some(count(ctx, memory, &filters))),
      Read::Failed(_) => (true, None),
      Read::Reading => (false, None),
    };
    let toolbar = (searchable || counted.is_none()).then(|| toolbar(ctx, &state, searchable, counted));
    let (list, side) = match read {
      Read::Reading => {
        // No loading frame draws a task's memory: its records' rows are bones in one outline until they are read.
        let bones = skeleton::list(
          ctx,
          "memory-reading-rows",
          &[(220.0, 320.0), (180.0, 280.0), (240.0, 300.0)],
        );
        let reading = Skeleton::new("memory-loading", ctx.t("memory.reading")).show(ctx, bones);
        (Shown::Scrolls(reading), None)
      }
      Read::Failed(reason) => (
        Shown::Scrolls(self.failed(ctx, &state, &reason)),
        Some(next::panel(ctx, None)),
      ),
      Read::Ready(memory) => {
        let side = next::panel(ctx, Some(&memory));
        (records(ctx, &state, &memory), Some(side))
      }
    };
    // A state in place of records stands in the middle of the list's room (`n3lxin`), which a scroll view's
    // content can't fill.
    let list = match list {
      Shown::Scrolls(list) => ScrollVertical::new(list)
        .id("task-memory-scroll")
        .width(Dimension::Pct(100.0))
        .height(Dimension::Pct(100.0))
        .into(),
      Shown::Fills(list) => list,
    };
    Column::new()
      .id("task-memory")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .with_children(toolbar)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .child(
            Column::new()
              .flex(1.0)
              .min_width(Dimension::Px(0.0))
              .height(Dimension::Pct(100.0))
              .child(list),
          )
          .with_children(side),
      )
  }
}

impl TaskMemoryTab {
  /// *Couldn't read the memory of KON-12*, why, and *Try again* (`AlQy0`).
  fn failed(&self, ctx: &Ctx, state: &MemoryState, reason: &Message) -> Element {
    let retries = self.retries.clone();
    let key = state.task.as_ref().map(|task| task.key.to_string()).unwrap_or_default();
    let retry = ActionButton::new("memory-retry", ButtonKind::Secondary, ctx.t("memory.retry"))
      .icon(Icon::RotateCw)
      .on_click(move || retries.update(|count| *count += 1));
    list_frame()
      .child(
        EmptyState::new(
          "memory-load-error",
          Icon::CircleAlert,
          ctx.t_args("memory.task.loadFailed", [("task", key.as_str())]),
          ctx.t_args("memory.task.loadFailedBody", [("reason", message_text(ctx, reason))]),
        )
        .unframed()
        .action(retry),
      )
      .into()
  }
}

/// What the tab shows of the task's memory, read off the UI thread whenever the project's tasks are saved or the
/// task's runs change, and when the person tries again.
enum Read {
  Reading,
  Ready(Arc<TaskMemory>),
  Failed(Message),
}

fn read(ctx: &mut Ctx, task: &TaskContext, retries: u64) -> Read {
  let feed = &task.project.feed;
  let live = task
    .detail
    .run
    .as_ref()
    .is_some_and(|run| !run.run.status().is_terminal());
  let again = feed.revision.get() + feed.run_changes(live) + retries;
  let hold = Services::of(ctx).hold;
  let loaded = ctx
    .future(
      (task.project.open.repo_root.clone(), task.detail.task.id(), again),
      move |(repo, id, _)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || TaskMemory::load_in(&repo, id).map(Shared::new))
            .await?
            .map_err(|error| messages::store_error(&error))
        }
      },
    )
    .state()
    .get();
  if let Some(error) = loaded.error.clone().filter(|_| loaded.is_rejected()) {
    return Read::Failed(error);
  }
  loaded.data.map_or(Read::Reading, |read| Read::Ready(read.0))
}

/// The record list's column (`sglU2`).
fn list_frame() -> Column {
  Column::new()
    .id("memory-list")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(MemorySpacing::ListSide.role(), SpacingSize::Lg))
    .spacing(MemorySpacing::Groups.role())
}

/// The list as the tab lays it out: in a scroll view, or filling its room while a state stands alone in it.
enum Shown {
  Scrolls(Element),
  Fills(Element),
}

/// The toolbar (`yFMam`): the search and the filters while there are records to search, the count, and *New
/// record*.
fn toolbar(ctx: &mut Ctx, state: &MemoryState, searchable: bool, counted: Option<Arc<str>>) -> Element {
  let open = state.clone();
  let mut bar = Row::new()
    .id("memory-toolbar")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .flex_shrink(0.0)
    .padding(Padding::horizontal(SpacingSize::Section))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
  if searchable {
    bar = bar.child(ctx.mount_keyed::<FilterBar>(
      "memory-task-filters",
      FiltersProps {
        project: false,
        tags: Arc::from([]),
      },
    ));
  }
  bar
    .child(Spacer::new().flex(1.0))
    .with_children(counted.map(|counted| {
      Text::new(&counted)
        .id("memory-count")
        .variant(ExtraTypography::Note.role())
        .nowrap()
    }))
    .child(
      ActionButton::new("memory-new", ButtonKind::Secondary, ctx.t("memory.newRecord"))
        .icon(Icon::Plus)
        .on_click(move || open.open(Opened::New)),
    )
    .into()
}

/// The task's records, or what the tab says while it has none (`sglU2`, `e8oWP4`) or none match (`fQ33n`).
fn records(ctx: &mut Ctx, state: &MemoryState, memory: &TaskMemory) -> Shown {
  let filters = state.filters.get();
  let mut above: Vec<Element> = memory_ui::new_record(ctx, state).into_iter().collect();
  if memory.unreadable > 0 {
    above.push(memory_ui::unreadable(ctx, memory.unreadable));
  }
  let alone = |above: Vec<Element>, state_view: Element| {
    if above.is_empty() {
      Shown::Fills(list_frame().height(Dimension::Pct(100.0)).child(state_view).into())
    } else {
      Shown::Scrolls(list_frame().with_children(above).child(state_view).into())
    }
  };
  if memory.records.is_empty() {
    return alone(above, empty(ctx, state, memory));
  }
  above.extend(memory_ui::row_failure(ctx, state, &memory.records, &filters));
  let groups = memory_ui::groups(
    ctx,
    state,
    ListInput {
      records: &memory.records,
      origins: &memory.origins,
      filters: &filters,
      task_tab: true,
      project: None,
    },
  );
  if groups.is_empty() {
    return alone(above, memory_ui::no_match(ctx, state, &memory.records, &filters));
  }
  Shown::Scrolls(list_frame().with_children(above).with_children(groups).into())
}

/// *10 active · 1 waiting*; *10 active · 4 not given* while every status shows; *0 of 3 decisions* while a type is
/// searched.
fn count(ctx: &Ctx, memory: &TaskMemory, filters: &Filters) -> Arc<str> {
  let counts = memory.counts();
  if let Some(kind) = filters.kind.filter(|_| !filters.search.trim().is_empty()) {
    let found = listing::shown(&memory.records, filters);
    return ctx.t_args(
      "memory.toolbar.ofKind",
      [
        ("found", found.to_string()),
        ("count", listing::of_kind(&memory.records, filters).to_string()),
        ("kind", words::kind_plural(ctx, kind).to_lowercase()),
      ],
    );
  }
  if filters.status == StatusFilter::All {
    return ctx.t_args(
      "memory.toolbar.notGiven",
      [
        ("active", counts.active.to_string()),
        ("count", counts.not_given().to_string()),
      ],
    );
  }
  ctx.t_args(
    "memory.toolbar.counts",
    [
      ("active", counts.active.to_string()),
      ("waiting", counts.waiting.to_string()),
    ],
  )
}

/// *No memory for this task yet* (`DDHrV` in `e8oWP4`): what agents keep, the step that works, and *New record*.
fn empty(ctx: &Ctx, state: &MemoryState, memory: &TaskMemory) -> Element {
  let mut body = ctx.t("memory.task.emptyBody").to_string();
  if let Some(next) = &memory.next {
    let step = words::step(ctx, next.step.number, &next.step.name);
    body = format!("{body} {}", ctx.t_args("memory.task.emptyWorking", [("step", &*step)]));
  }
  let open = state.clone();
  Centered::new("memory-empty", Icon::Brain, ctx.t("memory.task.emptyTitle"), body)
    .action(
      ActionButton::new("memory-empty-new", ButtonKind::Secondary, ctx.t("memory.newRecord"))
        .icon(Icon::Plus)
        .on_click(move || open.open(Opened::New)),
    )
    .into()
}

/// How many active records task `task` has, for the Memory tab's label; `None` until it is read, and once a
/// reading failed (the count read before may be out of date).
pub fn active_count(ctx: &mut Ctx, task: &TaskContext) -> Option<usize> {
  let feed = &task.project.feed;
  let again = feed.revision.get() + feed.states.get();
  let hold = Services::of(ctx).hold;
  let id: TaskId = task.detail.task.id();
  let counted = ctx
    .future(
      (task.project.open.repo_root.clone(), id, again),
      move |(repo, id, _)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || active_count_in(&repo, id))
            .await?
            .map_err(|error| messages::store_error(&error))
        }
      },
    )
    .state()
    .get();
  if counted.is_rejected() {
    return None;
  }
  counted.data
}
