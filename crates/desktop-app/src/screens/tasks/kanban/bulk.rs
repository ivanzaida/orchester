//! Several cards at once (`08 Multi-select` `fa4ib`, `13 Menus, bulk and header` `CdMab`): Shift+click or X selects
//! a card, and while any is selected a bar floats at the board's foot, `surface_raised` in a `border_strong` stroke,
//! radius 10: *3 selected*, Move to, Priority, Tags, Mark as done, Cancel task and Delete…, then Esc's cap and the
//! cross that clears the selection. Each asks or refuses as a single card's move does: Move to moves the cards that
//! can go and says which stay and why (a card whose run has not ended moves only to Done or Canceled), and asks first
//! when queuing drafts on Todo starts them at once; Mark as done asks first when work of a card waits to be merged;
//! Mark as done and Cancel task ask once to stop the runs that have not ended, naming them, then close every card;
//! Delete… asks once over the columns, by a person only, and is unavailable on a shared project. A read-only card
//! (`Dropping::locked`) is never selected.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle},
  },
  components::{Column, Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::RunId, task::organization::ChangeActor};
use orchester_store::ProjectDb;
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::{
    cancel_run::{Stop, StopRefused},
    close::{Closing, close_in},
    delete::delete_in,
    moves,
  },
  workspace::OpenProject,
};

use super::{
  actions::{Ask, BoardState, Failed, Selected, actor_now},
  card::CardModel,
  drop::Dropping,
};
use crate::{
  project::ProjectFeed,
  screens::task_close::Subject,
  services::Services,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    icon_button::IconButton,
    kbd::key_cap,
    menu::menu_panel,
  },
};

/// The bar (`Bulk bar` `vTiQj`), its dividers (`TE0rm`) and a bulk menu's width (`Y27GJ`).
const BAR_HEIGHT: f32 = 40.0;
const DIVIDER: (f32, f32) = (1.0, 20.0);
const MENU_WIDTH: f32 = 260.0;

/// What a bulk command asks of the project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bulk {
  /// Reads which of the tasks have a run that has not ended, to ask once before closing them all.
  Prepare(Closing, Vec<Subject>),
  /// Closes the tasks, stopping the runs the person confirmed stopping.
  Close(Closing, Vec<Subject>, Vec<(Subject, RunId)>),
  Delete(Vec<Subject>),
}

pub type BulkAction = FutureAction<Bulk, (), Failed>;

/// Carries out bulk commands; the tasks are read again after each, and the selection clears once it is done. Who asks
/// is read as the command is given, in the click or key that gave it: lurq takes any later work for the person's.
pub fn bulk_action(
  ctx: &mut Ctx,
  (project, feed): (OpenProject, ProjectFeed),
  (asked, failed, selected): (Signal<Option<Ask>>, Signal<Option<Failed>>, Signal<Selected>),
) -> BulkAction {
  let services = Services::of(ctx);
  ctx.future_action(move |bulk: Bulk| {
    let (project, feed, services) = (project.clone(), feed.clone(), services.clone());
    let (asked, failed, selected) = (asked.clone(), failed.clone(), selected.clone());
    let actor = actor_now();
    async move {
      let done = match bulk {
        Bulk::Prepare(closing, tasks) => {
          let runs = active_runs(&project, &tasks).await?;
          if !runs.is_empty() {
            asked.set(Some(Ask::BulkClose { closing, tasks, runs }));
            return Ok(());
          }
          close_all(&project, &services, (closing, &tasks, &[]), actor).await
        }
        Bulk::Close(closing, tasks, runs) => close_all(&project, &services, (closing, &tasks, &runs), actor).await,
        Bulk::Delete(tasks) if actor == ChangeActor::Person => delete_all(&project, &services, &tasks).await,
        Bulk::Delete(tasks) => Err(Failed {
          key: keys(&tasks),
          message: moves::delete_person_only(),
        }),
      };
      feed.saved();
      match &done {
        Ok(()) => selected.set(Selected::default()),
        Err(failure) => failed.set(Some(failure.clone())),
      }
      done
    }
  })
}

/// Closes the selected `tasks` as `closing`; marking them as done asks first, as a single card's Done does, when the
/// work of `unmerged` of them waits to be merged.
pub fn close_selected(state: &BoardState, closing: Closing, (tasks, unmerged): (Vec<Subject>, Vec<Subject>)) {
  if closing == Closing::Done && !unmerged.is_empty() {
    state.asked.set(Some(Ask::BulkUnmerged { tasks, unmerged }));
  } else {
    state.bulk.run(Bulk::Prepare(closing, tasks));
  }
}

/// The keys of `tasks`, joined as identifiers are.
fn keys(tasks: &[Subject]) -> Arc<str> {
  Arc::from(tasks.iter().map(|task| &*task.key).collect::<Vec<_>>().join(", "))
}

/// The runs of `tasks` that have not ended, as the store decides it.
async fn active_runs(project: &OpenProject, tasks: &[Subject]) -> Result<Vec<(Subject, RunId)>, Failed> {
  let (root, read) = (project.repo_root.clone(), tasks.to_vec());
  background::run(move || -> Result<Vec<(Subject, RunId)>, Message> {
    let db = ProjectDb::open(&root).map_err(|error| messages::store_error(&error))?;
    let mut runs = Vec::new();
    for task in read {
      if let Some(run) = db
        .active_task_run(task.id)
        .map_err(|error| messages::store_error(&error))?
      {
        runs.push((task, run));
      }
    }
    Ok(runs)
  })
  .await
  .and_then(|found| found)
  .map_err(|message| Failed {
    key: keys(tasks),
    message,
  })
}

/// Closes every task for `actor`, stopping first the run the person confirmed stopping (one that started since is not
/// stopped, and its task stays); the first that does not close says why.
async fn close_all(
  project: &OpenProject,
  services: &Services,
  (closing, tasks, runs): (Closing, &[Subject], &[(Subject, RunId)]),
  actor: ChangeActor,
) -> Result<(), Failed> {
  for task in tasks {
    let shown = runs.iter().find(|(held, _)| held.id == task.id).map(|(_, run)| *run);
    close_in(
      project,
      (&services.runners, &services.servers),
      task.id,
      (closing, Stop::Confirmed(shown), actor),
    )
    .await
    .map_err(|refused: StopRefused| Failed {
      key: task.key.clone(),
      message: refused.message(),
    })?;
  }
  Ok(())
}

async fn delete_all(project: &OpenProject, services: &Services, tasks: &[Subject]) -> Result<(), Failed> {
  for task in tasks {
    let (root, programs, id) = (project.repo_root.clone(), services.programs.clone(), task.id);
    background::run(move || delete_in(&root, &programs, id, false))
      .await
      .and_then(|deleted| deleted)
      .map_err(|message| Failed {
        key: task.key.clone(),
        message,
      })?;
  }
  Ok(())
}

/// Which list of the bar is open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BarMenu {
  #[default]
  None,
  MoveTo,
  Priority,
  Tags,
}

impl DevtoolsInspectable for BarMenu {}

#[derive(Clone, Debug, PartialEq)]
pub struct BarProps {
  /// The selected cards the board shows.
  pub cards: Vec<CardModel>,
}

impl DevtoolsInspectable for BarProps {}

pub struct BulkBar {
  menu: Signal<BarMenu>,
  open: Signal<bool>,
  anchors: [ElementRef; 3],
}

impl Component for BulkBar {
  type Props = BarProps;

  fn create(ctx: &mut Ctx) -> Self {
    let (menu, open) = (ctx.signal(BarMenu::None), ctx.signal(false));
    let closed = menu.clone();
    ctx.watch(&open, move |open: &bool| {
      if !*open {
        closed.set(BarMenu::None);
      }
    });
    Self {
      menu,
      open,
      anchors: Default::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let cards = ctx.props::<BarProps>().cards.clone();
    let dropping = Dropping::of(ctx);
    let menu = self.menu.get();
    if (menu != BarMenu::None) != self.open.get_untracked() {
      self.open.set(menu != BarMenu::None);
    }
    let popup = self.list(ctx, &dropping, &cards, menu);
    self.bar(ctx, &dropping, &cards).with_children(popup)
  }
}

impl BulkBar {
  fn bar(&self, ctx: &mut Ctx, dropping: &Dropping, cards: &[CardModel]) -> Row {
    let state = &dropping.state;
    let busy = state.busy() || state.bulk.state().get().is_pending() || dropping.closer.busy();
    let subjects: Vec<Subject> = cards.iter().map(|card| dropping.subject(card)).collect();
    let unmerged: Vec<Subject> = cards
      .iter()
      .filter(|card| moves::leaves_unmerged(&card.row))
      .map(|card| dropping.subject(card))
      .collect();
    let opener = |name: &str, (label, glyph): (&str, Icon), which: BarMenu, anchor: &ElementRef| {
      let menu = self.menu.clone();
      let button = ActionButton::new(format!("tasks-board-bulk-{name}"), ButtonKind::Ghost, ctx.t(label))
        .icon(glyph)
        .element_ref(anchor.clone());
      if busy {
        button.disabled()
      } else {
        button.on_click(move || menu.set(which))
      }
    };
    let closing = |name: &str, (label, glyph): (&str, Icon), closing: Closing| {
      let button = ActionButton::new(format!("tasks-board-bulk-{name}"), ButtonKind::Ghost, ctx.t(label)).icon(glyph);
      if busy {
        return button.disabled();
      }
      let (state, tasks, unmerged) = (state.clone(), subjects.clone(), unmerged.clone());
      button.on_click(move || close_selected(&state, closing, (tasks.clone(), unmerged.clone())))
    };
    let delete = delete_button(ctx, dropping, busy, &subjects);
    let selected = state.selected.clone();
    let clear = IconButton::new("tasks-board-bulk-clear", Icon::X, ctx.t("tasks.board.bulk.clear"))
      .small()
      .on_click(move || selected.set(Selected::default()));
    bar_frame()
      .child(
        Text::new(&ctx.t_args("tasks.board.bulk.selected", [("count", cards.len().to_string())]))
          .id("tasks-board-bulk-count")
          .variant(ExtraTypography::InspectorSection.role())
          .nowrap(),
      )
      .child(divider())
      .child(opener(
        "move",
        ("tasks.board.bulk.moveTo", Icon::ArrowRightLeft),
        BarMenu::MoveTo,
        &self.anchors[0],
      ))
      .child(opener(
        "priority",
        ("tasks.board.bulk.priority", Icon::SignalHigh),
        BarMenu::Priority,
        &self.anchors[1],
      ))
      .child(opener(
        "tags",
        ("tasks.board.bulk.tags", Icon::Tag),
        BarMenu::Tags,
        &self.anchors[2],
      ))
      .child(closing(
        "done",
        ("task_close.menu.done", Icon::CircleCheck),
        Closing::Done,
      ))
      .child(closing(
        "cancel",
        ("task_close.menu.cancel", Icon::CircleX),
        Closing::Canceled,
      ))
      .child(delete)
      .child(divider())
      .child(key_cap("tasks-board-bulk-esc", &ctx.t("tasks.board.keys.esc")))
      .child(clear.render(ctx))
  }

  /// The bar's open list, over the button that opened it.
  fn list(&self, ctx: &mut Ctx, dropping: &Dropping, cards: &[CardModel], menu: BarMenu) -> Option<Popup> {
    let (rows, anchor) = match menu {
      BarMenu::None => return None,
      BarMenu::MoveTo => (super::bulk_menus::move_rows(ctx, dropping, cards), &self.anchors[0]),
      BarMenu::Priority => (super::bulk_menus::priority_rows(ctx, dropping, cards), &self.anchors[1]),
      BarMenu::Tags => (super::bulk_menus::tag_rows(ctx, dropping, cards), &self.anchors[2]),
    };
    let rows: Vec<Element> = rows.into_iter().map(|row| row.render(ctx, &self.open)).collect();
    Some(
      Popup::new(
        anchor.clone(),
        menu_panel("tasks-board-bulk-menu")
          .width(Dimension::Px(MENU_WIDTH))
          .with_children(rows),
      )
      .open(self.open.clone())
      .placement(Placement::TopStart)
      .offset(0.0, -ExtraSpacing::Snug.value()),
    )
  }
}

/// Delete… for the selection: unavailable on a shared project, asked once over the columns otherwise.
fn delete_button(ctx: &Ctx, dropping: &Dropping, busy: bool, subjects: &[Subject]) -> ActionButton {
  let delete = ActionButton::new(
    "tasks-board-bulk-delete",
    ButtonKind::DangerGhost,
    ctx.t("tasks.board.bulk.delete"),
  )
  .icon(Icon::Trash2);
  match () {
    () if dropping.shared => delete.unavailable(ctx.t("task_close.menu.deleteShared")),
    () if busy => delete.disabled(),
    () => {
      let (asked, tasks) = (dropping.state.asked.clone(), subjects.to_vec());
      delete.on_click(move || asked.set(Some(Ask::BulkDelete(tasks.clone()))))
    }
  }
}

/// The floating bar's frame (`Bulk bar` `vTiQj`).
fn bar_frame() -> Row {
  Row::new()
    .id("tasks-board-bulk")
    .height(Dimension::Px(BAR_HEIGHT))
    .padding(
      Padding::new()
        .left(ExtraSpacing::Inset.role())
        .right(ExtraSpacing::Snug.role()),
    )
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Lg)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .box_shadow(ShadowStyle::Md)
}

fn divider() -> Column {
  Column::new()
    .width(Dimension::Px(DIVIDER.0))
    .height(Dimension::Px(DIVIDER.1))
    .background(PaletteColor::BorderStrong)
}
