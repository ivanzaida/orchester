//! A card's menu (`Card menu` `vV3BX` in `07 Card actions and keys` `pXzH8`): Open, Move to, Priority and Tags, which
//! each open their list beside the menu (`Move to menu` `c88il`), Copy key, Run again for a task whose run ended, Mark
//! as done, Cancel task or Reopen task as its status allows, and Delete task… in `danger`, which asks at the card
//! (unavailable on a shared project). Move to lists the columns, each one a card cannot go to faded, as a drop refuses
//! it, and says why once in its foot (`Move to for a card whose run has not ended` `kn5MV`). While a change is under
//! way the menu's changes wait (`Menu while a change is under way` `ogTTt`). Once the person is no longer a member of
//! the project, the menu is kept with Open and Copy key only.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::{Ctx, OutsidePress},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Placement, Popup, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  project::TaskKey,
  task::organization::{Priority, WorkflowStatus},
};
use orchester_workspace::tasks::moves::{self, Decision};

use super::{
  actions::{BoardState, Change, ChangeKind, MenuPage, OpenMenu, Target},
  card::CardModel,
  drop::Dropping,
  model::ColumnKey,
  refusal::reason_key,
};
use crate::{
  navigation::{Navigation, press::OpenIn},
  routes::{ProjectPage, Route, TaskView},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    clipboard::Clipboard,
    icon::Icon,
    menu::{MenuItem, menu_panel, separator},
    status::{priority_look, workflow_key, workflow_look},
  },
};

/// The menu's width (`vV3BX`) and its lists' (`c88il`); a list opens 8 past the end of the row that opens it and 9
/// over its top, so its first row sits beside that row.
const MENU_WIDTH: f32 = 220.0;
const LIST_WIDTH: f32 = 200.0;
const LIST_OFFSET: (f32, f32) = (8.0, -9.0);

/// What a card's menu opens from: whether it is open and what it opens beside, and the same for its list, which opens
/// beside the row that opens it.
pub struct MenuHandles<'a> {
  pub open: &'a Signal<bool>,
  pub anchor: &'a ElementRef,
  pub list_open: &'a Signal<bool>,
  pub list_anchor: &'a ElementRef,
}

/// The menu of `card`, open while `handles.open` holds, beside `handles.anchor`, with the list it opened beside it.
pub fn card_menu(ctx: &mut Ctx, card: &CardModel, handles: &MenuHandles<'_>) -> Popup {
  let state = BoardState::of(ctx);
  let page = state
    .menu
    .get()
    .0
    .filter(|(task, _)| *task == card.row.id)
    .map_or(MenuPage::Commands, |(_, page)| page);
  let id = format!("task-menu-{}", card.row.key);
  let open = handles.open;
  let rows = commands(ctx, card, &state, (open, page, handles.list_anchor));
  let list = match page {
    MenuPage::Commands => None,
    MenuPage::MoveTo => Some(move_to(ctx, card, open)),
    MenuPage::Priority => Some(priorities(ctx, card, &state, open)),
    MenuPage::Tags => Some(tags(ctx, card, &state, open)),
  };
  // A press elsewhere in the menu closes the list and reaches the row it is on.
  let list = list.map(|rows| {
    let panel = menu_panel(&format!("{id}-list"))
      .width(Dimension::Px(LIST_WIDTH))
      .with_children(rows);
    Popup::new(handles.list_anchor.clone(), panel)
      .open(handles.list_open.clone())
      .placement(Placement::RightStart)
      .offset(LIST_OFFSET.0, LIST_OFFSET.1)
      .outside_press(OutsidePress::PassThrough)
      .switch_on_anchor_press(false)
  });
  let panel = menu_panel(&id)
    .width(Dimension::Px(MENU_WIDTH))
    .with_children(rows)
    .with_children(list);
  Popup::new(handles.anchor.clone(), panel)
    .open(open.clone())
    .placement(Placement::BottomEnd)
    .offset(0.0, ExtraSpacing::Snug.value())
}

/// Opens the list `page` of `card`'s menu beside it.
fn opens(
  state: &BoardState,
  open: &Signal<bool>,
  card: &CardModel,
  page: MenuPage,
) -> impl Fn() + Send + Sync + 'static {
  let (menu, open, task) = (state.menu.clone(), open.clone(), card.row.id);
  move || {
    menu.set(OpenMenu(Some((task, page))));
    open.set(true);
  }
}

/// The menu's commands; a read-only card's (`Dropping::locked`) only opens its task and copies its key. The row of the
/// open list `page` is what that list opens beside.
fn commands(
  ctx: &mut Ctx,
  card: &CardModel,
  state: &BoardState,
  (open, page, list_anchor): (&Signal<bool>, MenuPage, &ElementRef),
) -> Vec<Element> {
  let dropping = Dropping::of(ctx);
  let busy = state.busy() || dropping.closer.busy();
  let id = |name: &str| format!("task-menu-{}-{name}", card.row.key);
  let task: TaskKey = card.row.key.parse().expect("a task's key reads back as a task key");
  let navigation = Navigation::of(ctx);
  let route = Route::Project(card.project, ProjectPage::Task(task, TaskView::default()));
  let open_task = MenuItem::new(id("open"), ctx.t("tasks.board.menu.open"), move || {
    navigation.open(route.clone(), OpenIn::ActiveTab);
  })
  .icon(Icon::ArrowUpRight)
  .shortcut(ctx.t("tasks.board.keys.enter"));
  let waiting = ctx.t("task_close.menu.busy");
  let changing = |name: &str, label: Arc<str>, glyph: Icon, list: MenuPage| {
    if busy {
      return MenuItem::unavailable(id(name), label, waiting.clone()).icon(glyph);
    }
    MenuItem::new(id(name), label, opens(state, open, card, list))
      .icon(glyph)
      .submenu()
  };
  let mut rows: Vec<Element> = vec![open_task.render(ctx, open), separator(ctx)];
  if dropping.locked(card) {
    rows.push(copy_key(ctx, card, open));
    return rows;
  }
  for (name, key, glyph, list, shortcut) in [
    (
      "move",
      "tasks.board.menu.moveTo",
      Icon::ArrowRightLeft,
      MenuPage::MoveTo,
      "tasks.board.keys.move",
    ),
    (
      "priority",
      "tasks.board.menu.priority",
      Icon::SignalHigh,
      MenuPage::Priority,
      "tasks.board.keys.priority",
    ),
    (
      "tags",
      "tasks.board.menu.tags",
      Icon::Tag,
      MenuPage::Tags,
      "tasks.board.keys.tags",
    ),
  ] {
    let (label, keys) = (ctx.t(key), ctx.t(shortcut));
    let row = changing(name, label, glyph, list).shortcut(keys).render(ctx, open);
    rows.push(if list == page {
      Column::new()
        .width(Dimension::Pct(100.0))
        .ref_element(list_anchor.clone())
        .child(row)
        .into()
    } else {
      row
    });
  }
  rows.push(copy_key(ctx, card, open));
  rows.extend(run_again(ctx, card, (&dropping, busy), open));
  rows.push(separator(ctx));
  rows.extend(closing(ctx, card, (&dropping, busy), open));
  rows
}

/// Copy key, with its keys.
fn copy_key(ctx: &mut Ctx, card: &CardModel, open: &Signal<bool>) -> Element {
  let clipboard = ctx.use_context::<Clipboard>().unwrap_or_default();
  let copied = card.row.key.clone();
  MenuItem::new(
    format!("task-menu-{}-copy", card.row.key),
    ctx.t("tasks.board.menu.copyKey"),
    move || {
      if !clipboard.copy(&copied) {
        tracing::warn!("the task's key was not copied");
      }
    },
  )
  .icon(Icon::Copy)
  .shortcut(ctx.t(if cfg!(target_os = "macos") {
    "tasks.board.keys.copyMac"
  } else {
    "tasks.board.keys.copy"
  }))
  .render(ctx, open)
}

/// Run again, for a task whose latest run ended: asked at the card first, as a drop on Todo asks, also for a card in
/// Todo already, which a drop there leaves as it is.
fn run_again(
  ctx: &mut Ctx,
  card: &CardModel,
  (dropping, busy): (&Dropping, bool),
  open: &Signal<bool>,
) -> Vec<Element> {
  if !card.row.has_run || card.row.run_not_ended || card.row.not_shared {
    return Vec::new();
  }
  let id = format!("task-menu-{}-run-again", card.row.key);
  let label = ctx.t("tasks.board.menu.runAgain");
  let row = if busy {
    MenuItem::unavailable(id, label, ctx.t("task_close.menu.busy"))
  } else {
    let (dropping, card) = (dropping.clone(), card.clone());
    MenuItem::new(id, label, move || {
      if !dropping.locked(&card) {
        dropping.state.refused.set(None);
        dropping.ask_run_again(&card);
      }
    })
  };
  vec![separator(ctx), row.icon(Icon::RotateCcw).render(ctx, open)]
}

/// Mark as done and Cancel task, or Reopen task for a closed one, and Delete task… in `danger`, through the screen's
/// closer.
fn closing(ctx: &mut Ctx, card: &CardModel, (dropping, busy): (&Dropping, bool), open: &Signal<bool>) -> Vec<Element> {
  let subject = dropping.subject(card);
  let id = |name: &str| format!("task-menu-{}-{name}", card.row.key);
  let waiting = ctx.t("task_close.menu.busy");
  let item = |name: &str, (label, glyph): (Arc<str>, Icon), choose: Box<dyn Fn() + Send + Sync>| {
    if busy {
      return MenuItem::unavailable(id(name), label, waiting.clone()).icon(glyph);
    }
    MenuItem::new(id(name), label, choose).icon(glyph)
  };
  let to = |status: WorkflowStatus| -> Box<dyn Fn() + Send + Sync> {
    let (dropping, card) = (dropping.clone(), card.clone());
    Box::new(move || {
      let target = Target {
        column: ColumnKey::Status(status),
        lane: None,
      };
      dropping.drop_card(&card, &target);
    })
  };
  let done = (ctx.t("task_close.menu.done"), Icon::CircleCheck);
  let cancel = (ctx.t("task_close.menu.cancel"), Icon::CircleSlash);
  let reopen = || -> Box<dyn Fn() + Send + Sync> {
    let (closer, subject) = (dropping.closer.clone(), subject.clone());
    Box::new(move || closer.reopen(subject.clone()))
  };
  let mut items = match card.row.organization.status() {
    WorkflowStatus::Done => vec![
      item("reopen", (ctx.t("task_close.menu.reopen"), Icon::RotateCcw), reopen()),
      item("cancel", cancel, to(WorkflowStatus::Canceled)),
    ],
    WorkflowStatus::Canceled => vec![
      item("reopen", (ctx.t("task_close.menu.reopen"), Icon::RotateCcw), reopen()),
      item("done", done, to(WorkflowStatus::Done)),
    ],
    _ => vec![
      item("done", done, to(WorkflowStatus::Done)),
      item("cancel", cancel, to(WorkflowStatus::Canceled)),
    ],
  };
  let delete = ctx.t("task_close.menu.delete");
  items.push(if subject.shared {
    MenuItem::unavailable(id("delete"), delete, ctx.t("task_close.menu.deleteShared"))
  } else {
    let closer = dropping.closer.clone();
    MenuItem::new(id("delete"), delete, move || closer.ask_delete(subject.clone()))
  });
  let last = items.len() - 1;
  items
    .into_iter()
    .enumerate()
    .map(|(index, item)| {
      let item = if index == last {
        item.icon(Icon::Trash2).danger()
      } else {
        item
      };
      item.render(ctx, open)
    })
    .collect()
}

/// The columns by status, each one the card cannot go to faded, and why in the list's foot.
fn move_to(ctx: &mut Ctx, card: &CardModel, open: &Signal<bool>) -> Vec<Element> {
  let dropping = Dropping::of(ctx);
  let mut rows: Vec<Element> = Vec::new();
  let mut refused: Vec<(Arc<str>, moves::Refusal)> = Vec::new();
  for status in WorkflowStatus::ALL {
    if status == WorkflowStatus::Canceled {
      rows.push(separator(ctx));
    }
    let id = format!("task-menu-{}-move-{}", card.row.key, super::model::status_name(status));
    let label = ctx.t(workflow_key(status));
    let glyph = workflow_look(status).0;
    let item = match moves::decide(&card.row, status) {
      Decision::Stay => MenuItem::new(id, label, || {}).chosen(true),
      Decision::Refused(refusal) => {
        refused.push((label.clone(), refusal));
        MenuItem::unavailable(id, label, ctx.t(reason_key(refusal))).reason_below()
      }
      Decision::Move(_) => {
        let (dropping, card) = (dropping.clone(), card.clone());
        MenuItem::new(id, label, move || {
          let target = Target {
            column: ColumnKey::Status(status),
            lane: None,
          };
          dropping.drop_card(&card, &target);
        })
      }
    };
    rows.push(item.icon(glyph).render(ctx, open));
  }
  if !refused.is_empty() {
    rows.push(separator(ctx));
    rows.push(reasons(ctx, card, &refused));
  }
  rows
}

/// Why the faded columns of Move to refuse the card, once for each reason: *Backlog, Todo: This task already has a
/// run.*, or for a run that has not ended that Done and Canceled stop it (`Reason` `G1SqVJ`, `eTnYg`).
fn reasons(ctx: &Ctx, card: &CardModel, refused: &[(Arc<str>, moves::Refusal)]) -> Element {
  let mut seen: Vec<moves::Refusal> = Vec::new();
  for (_, refusal) in refused {
    if !seen.contains(refusal) {
      seen.push(*refusal);
    }
  }
  let separator = ctx.t("tasks.listSeparator");
  let lines = seen.into_iter().map(|refusal| {
    let words = if refusal == moves::Refusal::RunNotEnded {
      ctx.t("tasks.board.menu.runNotEnded")
    } else {
      let columns: Vec<&str> = refused
        .iter()
        .filter(|(_, held)| *held == refusal)
        .map(|(label, _)| &**label)
        .collect();
      ctx.t_args(
        "tasks.board.menu.reason",
        [
          ("columns", columns.join(&separator)),
          ("reason", ctx.t(reason_key(refusal)).to_string()),
        ],
      )
    };
    Text::new(&words)
      .variant(ExtraTypography::Note.role())
      .color(PaletteColor::TextMuted)
      .width(Dimension::Pct(100.0))
  });
  Column::new()
    .id(format!("task-menu-{}-move-reason", card.row.key))
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Tight.role())
        .bottom(ExtraSpacing::Snug.role())
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm),
    )
    .with_children(lines)
    .into()
}

fn priorities(ctx: &mut Ctx, card: &CardModel, state: &BoardState, open: &Signal<bool>) -> Vec<Element> {
  let now = card.row.organization.priority();
  let mut rows: Vec<Element> = Vec::new();
  for priority in Priority::ALL {
    let (glyph, key, _) = priority_look(priority);
    let id = format!(
      "task-menu-{}-priority-{}",
      card.row.key,
      super::model::priority_name(priority)
    );
    let (state, task, key_of) = (state.clone(), card.row.id, card.row.key.clone());
    let item = MenuItem::new(id, ctx.t(key), move || {
      state.change(vec![Change {
        task,
        key: key_of.clone(),
        kind: ChangeKind::Priority(priority),
      }]);
    })
    .icon(glyph)
    .chosen(priority == now);
    rows.push(item.render(ctx, open));
  }
  rows
}

/// The project's tags, each chosen while the task has it; a choice adds or takes it off.
fn tags(ctx: &mut Ctx, card: &CardModel, state: &BoardState, open: &Signal<bool>) -> Vec<Element> {
  let dropping = Dropping::of(ctx);
  if dropping.tags.is_empty() {
    return vec![
      MenuItem::unavailable(
        format!("task-menu-{}-tags-none", card.row.key),
        ctx.t("tasks.board.menu.noTags"),
        ctx.t("tasks.board.menu.noTagsReason"),
      )
      .without_lock()
      .render(ctx, open),
    ];
  }
  let held = card.row.organization.tags().to_vec();
  let mut rows: Vec<Element> = Vec::new();
  for (index, (tag, name)) in dropping.tags.iter().enumerate() {
    let chosen = held.contains(tag);
    let mut next = held.clone();
    if chosen {
      next.retain(|held| held != tag);
    } else {
      next.push(*tag);
    }
    let (state, task, key) = (state.clone(), card.row.id, card.row.key.clone());
    let item = MenuItem::new(
      format!("task-menu-{}-tag-{index}", card.row.key),
      name.clone(),
      move || {
        state.change(vec![Change {
          task,
          key: key.clone(),
          kind: ChangeKind::Tags(next.clone()),
        }]);
      },
    )
    .icon(Icon::Tag)
    .chosen(chosen);
    rows.push(item.render(ctx, open));
  }
  rows
}
