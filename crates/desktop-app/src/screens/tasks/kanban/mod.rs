//! The Tasks board (ORC-105, `Tasks board · redesign` `J6gQo` in the new `orchester.pen`): the view's tasks as cards
//! in columns by workflow status (Backlog, Todo, In progress, In review and Done; Canceled once Display shows it), or
//! by priority, module or pipeline, in rows by module or priority when Display asks (`model`). A card opens its task;
//! a drag moves it to another column by the person's rules (`drop`, `orchester_workspace::tasks::moves`): refused
//! with why in the column, asked first in place over the columns, or closed through the screen's closer, whose
//! confirmation stops a run that has not ended. Its menu, the keys and the selection bar do the same (`menu`, `keys`,
//! `bulk`). Backlog adds drafts (`quick_add`). The list's header, views, filters, notices and close panel stand over
//! it, as the screen draws them (`super::TasksScreen::top`). lurq's drag events carry the drag; the lifted card, the
//! drop slot and the columns' look are drawn here, hit-tested against the columns as laid out (`actions`).
//!
//! The board is read-only where the list is: a project the person is no longer a member of lifts, moves and selects no
//! card and locks quick add, its menus kept with Open and Copy key; the *Not shared* tasks lift, move and select no
//! card and have no menu (`Dropping::locked`).

mod actions;
mod ask;
mod bulk;
mod bulk_menus;
mod card;
mod card_view;
mod column;
pub mod display;
mod drop;
mod keys;
mod lanes;
mod lines;
mod menu;
mod model;
pub mod options;
mod quick_add;
mod refusal;
pub mod skeleton;

use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
  },
  components::{Column, Row, Stack},
  core::Signal,
  layout::StackAlignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{TagId, TaskId},
  task::organization::WorkflowStatus,
  time::Timestamp,
};
use orchester_workspace::tasks::{attention::NeedItem, close::Closing, data::ProjectTasks, list::TaskRow};

use self::{
  actions::BoardState,
  card::{CardModel, CardSource},
  card_view::lifted_frame,
  column::{Shared, column, strip},
  display::{Columns, Display},
  drop::Dropping,
  keys::{Grid, KeyContext, on_key},
  model::{ColumnKey, Source},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::TaskListTab,
  screens::task_close::Closer,
  theme::{ExtraSpacing, TasksSpacing},
  ui::clipboard::Clipboard,
};

/// What the board is drawn from: the project, its view, the view's rows as searched, filtered and ordered and all of
/// them, what waits for the person, how they are shown, and what the screen knows of the project.
pub struct BoardInput<'a> {
  pub project: &'a ShownProject,
  pub view: TaskListTab,
  pub rows: &'a [TaskRow],
  pub every: &'a [TaskRow],
  pub tasks: &'a ProjectTasks,
  pub needs: &'a HashMap<TaskId, NeedItem>,
  pub highlight: Option<&'a str>,
  pub display: Signal<Display>,
  /// Free agent slots, while the runner serves the project and new starts are not held.
  pub free: Option<usize>,
  pub shared: bool,
  /// Why nothing can be added, once the person is no longer a member.
  pub read_only: Option<Arc<str>>,
  /// The *Not shared* filter is on: the cards are read-only.
  pub not_shared: bool,
  pub filtered: bool,
  /// What clears the search and the filters.
  pub clear: Arc<dyn Fn() + Send + Sync>,
}

/// The board, mounted for the project.
pub fn board(ctx: &mut Ctx, input: BoardInput<'_>) -> Element {
  let source = CardSource {
    project: input.project.open.project.id(),
    tasks: input.tasks,
    needs: input.needs,
    highlight: input.highlight,
    now: Timestamp::now(),
  };
  let cards: Vec<CardModel> = input.rows.iter().map(|row| CardModel::of(ctx, &source, row)).collect();
  let tags = input
    .tasks
    .tags
    .iter()
    .map(|tag| (tag.id(), Arc::from(tag.name().as_str())))
    .collect();
  ctx.mount_keyed::<Board>(
    &format!("tasks-board-{}", input.project.open.project.id()),
    BoardProps {
      project: input.project.clone(),
      view: input.view,
      cards: Arc::new(cards),
      every: Arc::new(input.every.to_vec()),
      tasks: input.tasks.clone(),
      display: input.display,
      free: input.free,
      shared: input.shared,
      read_only: input.read_only,
      not_shared: input.not_shared,
      filtered: input.filtered,
      tags: Arc::new(tags),
      clear: input.clear,
    },
  )
}

#[derive(Clone)]
struct BoardProps {
  project: ShownProject,
  view: TaskListTab,
  cards: Arc<Vec<CardModel>>,
  every: Arc<Vec<TaskRow>>,
  tasks: ProjectTasks,
  display: Signal<Display>,
  free: Option<usize>,
  shared: bool,
  read_only: Option<Arc<str>>,
  not_shared: bool,
  filtered: bool,
  tags: Arc<Vec<(TagId, Arc<str>)>>,
  clear: Arc<dyn Fn() + Send + Sync>,
}

/// Compared by what they show; the display by its signal, and what clears the filters not at all.
impl PartialEq for BoardProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.view == other.view
      && self.cards == other.cards
      && self.every == other.every
      && self.tasks == other.tasks
      && self.display.id() == other.display.id()
      && self.free == other.free
      && self.shared == other.shared
      && self.read_only == other.read_only
      && self.not_shared == other.not_shared
      && self.filtered == other.filtered
      && self.tags == other.tags
  }
}

impl DevtoolsInspectable for BoardProps {}

struct Board {
  state: BoardState,
}

impl Component for Board {
  type Props = BoardProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<BoardProps>().clone();
    Self {
      state: BoardState::new(ctx, (props.project.open, props.project.feed), props.display),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<BoardProps>().clone();
    let state = self.state.clone();
    let closer = Closer::of(ctx);
    let display = props.display.get();
    let canceled = closer
      .closed()
      .filter(|(_, closing)| *closing == Closing::Canceled)
      .map(|(task, _)| task);
    if canceled.is_some() && !state.canceled_now.get_untracked() {
      state.canceled_now.set(true);
    }
    let source = Source {
      view: props.view,
      rows: &props.cards,
      every: &props.every,
      tasks: &props.tasks,
      display: &display,
      canceled_now: state.canceled_now.get(),
    };
    let columns = model::columns(ctx, &source);
    let lanes = model::lanes(ctx, &display, (&props.tasks, &columns));
    let dropping = Dropping {
      state: state.clone(),
      closer: closer.clone(),
      lanes: display.lanes,
      free: props.free,
      shared: props.shared,
      read_only: props.read_only.is_some(),
      tags: props.tags.clone(),
    };
    ctx.provide(state.clone());
    ctx.provide(dropping.clone());
    state.clear_targets();
    let body = self.body(ctx, (&props, &display), (&columns, lanes.as_deref()));
    let lines = top_lines(ctx, (&state, &closer), canceled.as_ref());
    let keys = KeyContext {
      dropping,
      grid: grid(&columns),
      navigation: Navigation::of(ctx),
      clipboard: ctx.use_context::<Clipboard>().unwrap_or_default(),
    };
    let floating = self.floating(ctx, &props, &display);
    Stack::new()
      .id("tasks-board")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .stack_align(StackAlignment::BottomCenter)
      .ref_element(state.frame.clone())
      .describe("layout", "board")
      .on_key_down(move |event: KeyboardEvent| on_key(&keys, &event))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .height(Dimension::Pct(100.0))
          .padding(lurq::node::padding::Padding::new().right(TasksSpacing::Sides.role()))
          .spacing(ExtraSpacing::Loose.role())
          .with_children(lines)
          .child(body),
      )
      .with_children(floating)
  }
}

impl Board {
  /// The columns, or the rows of columns.
  fn body(
    &self,
    ctx: &mut Ctx,
    (props, display): (&BoardProps, &Display),
    (columns, lanes): (&[model::ColumnModel], Option<&[model::LaneModel]>),
  ) -> Element {
    let state = &self.state;
    let (drag, refused, selected) = (state.drag.get(), state.refused.get(), state.selected.get());
    let waiting = match state.asked.get() {
      Some(
        actions::Ask::StartsNow { task, .. } | actions::Ask::RunAgain { task, .. } | actions::Ask::Unmerged(task),
      ) => Some(task.id),
      _ => None,
    };
    let quick_add =
      (display.columns == Columns::Status && !props.not_shared).then(|| props.read_only.clone().map_or(Ok(()), Err));
    let shared = Shared {
      display,
      selected: &selected,
      focused: state.focused.get(),
      drag: drag.as_ref(),
      refused: refused.as_ref(),
      waiting,
      lifts: props.read_only.is_none() && !props.not_shared,
      menus: !props.not_shared,
      quick_add,
      filtered: props.filtered,
      clear: props.clear.clone(),
      project: &props.project,
    };
    if let Some(lanes) = lanes {
      return lanes::lanes(ctx, (state, &shared), columns, lanes);
    }
    let parts: Vec<Element> = columns
      .iter()
      .map(|column_model| {
        // Canceled shows folded until Display shows it: a task canceled from the board does not vanish.
        let folded = display.collapsed.contains(&column_model.key.name())
          || (column_model.key == ColumnKey::Status(WorkflowStatus::Canceled) && !display.show_canceled);
        if folded {
          strip(ctx, state, column_model, lurq::app::theme::PaletteColor::Border)
        } else {
          column(ctx, (state, &shared), (column_model, &column_model.cards), None)
        }
      })
      .collect();
    Row::new()
      .id("tasks-board-columns")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Loose.role())
      .with_children(parts)
      .into()
  }

  /// What floats over the columns: the lifted card under the pointer, and the hint or the selection bar at the foot.
  fn floating(&self, ctx: &mut Ctx, props: &BoardProps, display: &Display) -> Vec<Element> {
    let state = &self.state;
    let mut parts = Vec::new();
    if let Some(drag) = state.drag.get() {
      parts.push(ctx.mount_keyed::<Lifted>(
        "tasks-board-lifted",
        LiftedProps {
          drag: drag.clone(),
          display: display.clone(),
        },
      ));
      parts.push(foot(lines::hint(ctx, &drag)));
      return parts;
    }
    // A read-only card is never acted on together: one selected before the list became read-only is left out.
    let selected = state.selected.get();
    let cards: Vec<CardModel> = props
      .cards
      .iter()
      .filter(|card| selected.0.contains(&card.row.id) && props.read_only.is_none() && !card.row.not_shared)
      .cloned()
      .collect();
    if !cards.is_empty() {
      parts.push(foot(
        ctx.mount_keyed::<bulk::BulkBar>("tasks-board-bulk", bulk::BarProps { cards }),
      ));
    }
    parts
  }
}

/// `part` floating at the board's foot.
fn foot(part: impl Into<Element>) -> Element {
  Column::new().padding(lines::foot_room()).child(part).into()
}

/// What stands over the columns: why the last change did not happen, where a canceled task went, and what is asked in
/// place.
fn top_lines(
  ctx: &mut Ctx,
  (state, closer): (&BoardState, &Closer),
  canceled: Option<&crate::screens::task_close::Subject>,
) -> Vec<Element> {
  let mut lines = Vec::new();
  if let Some(failed) = state.failed.get() {
    lines.push(lines::failure(ctx, state, &failed));
  }
  if let Some(task) = canceled {
    lines.push(lines::canceled(ctx, (state, closer), task));
  }
  if let Some(asked) = state.asked.get() {
    lines.push(ask::ask_card(ctx, (state, closer), &asked));
  }
  lines
}

/// The cards by column, as the keys move through them.
fn grid(columns: &[model::ColumnModel]) -> Grid {
  Arc::new(
    columns
      .iter()
      .map(|column| (column.key.clone(), column.cards.clone()))
      .collect(),
  )
}

#[derive(Clone, Debug, PartialEq)]
struct LiftedProps {
  drag: actions::Drag,
  display: Display,
}

impl DevtoolsInspectable for LiftedProps {}

/// The card a drag lifted, under the pointer: it follows the pointer in a render of its own, so the board renders
/// again only when what the card is over changes.
struct Lifted;

impl Component for Lifted {
  type Props = LiftedProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LiftedProps>().clone();
    let state = BoardState::of(ctx);
    let (x, y) = state.pointer.get();
    let frame = state.frame.bounds();
    let (width, height) = props.drag.size;
    let look = card::CardLook::default();
    let parts = card::content(ctx, &props.drag.card, (&props.display, look), None);
    lifted_frame(parts).absolute(
      x - frame.x - props.drag.grab.0,
      y - frame.y - props.drag.grab.1,
      Dimension::Px(width),
      Dimension::Px(height),
    )
  }
}
