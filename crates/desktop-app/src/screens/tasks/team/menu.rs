//! A team task's row menu on the Tasks list (decision D4, `Task - v2` `jDN0r` T7 `lCoNt`, T8 `vdSWS`, T10 `tlw0r`):
//! Open and Copy key, *Move to* for an Admin on a task in Backlog or Todo (its list: Backlog and Todo, the current one
//! checked, *Runs move a task on from there.*), then only what the task's capabilities let the person do — Mark as
//! done, Mark as blocked or Clear blocked, Cancel task on an open task, *Reopen task* on a closed one (its list: Back
//! to Backlog and Back to Todo, *A reopened task is free: nobody holds it.*) — and a foot line saying who does the
//! rest. An item the person may not use is absent, never disabled, and a closed task offers no closing or blocking.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::{Ctx, OutsidePress},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ProjectId, TaskId},
  project::TaskKey,
  task::organization::{TaskOrganization, WorkflowStatus},
};

use super::actions::{Ask, TeamActions};
use crate::{
  navigation::{Navigation, press::OpenIn},
  routes::{ProjectPage, Route, TaskView},
  screens::tasks::kanban::copy_key_item,
  team::lifecycle::{Change, Offered},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    icon::Icon,
    menu::{MenuItem, menu_panel, separator},
    status::{workflow_key, workflow_look},
  },
};

/// The menu's width and its lists' (`vV3BX`, `c88il` in `07`, as T10's menus draw them); a list opens 8 past the end of
/// the row that opens it and 9 over its top, so its first row sits beside that row.
pub(in crate::screens::tasks) const MENU_WIDTH: f32 = 220.0;
const LIST_WIDTH: f32 = 200.0;
const LIST_OFFSET: (f32, f32) = (8.0, -9.0);

/// What a team task's row menu is made from: the task, as the list read it, and what it offers the person.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::screens::tasks) struct RowTeam {
  pub key: Arc<str>,
  pub id: TaskId,
  /// The task's organization as the list read it, which a change is made over.
  pub organization: TaskOrganization,
  /// `None` until the list read the task's capabilities.
  pub offered: Option<Offered>,
  /// Who changes the team's tasks, as the members read named them; empty until read, or for an Admin.
  pub admins: Arc<str>,
}

/// The lists a row's menu opens beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
  MoveTo,
  Reopen,
}

impl lurq::app::component::DevtoolsInspectable for Page {}

/// Which list of a row's menu is open, and what it opens beside.
#[derive(Clone)]
pub(in crate::screens::tasks) struct Lists {
  page: Signal<Option<Page>>,
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Lists {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      page: ctx.signal(None),
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }

  /// Opens `page` beside its row; the choice closes the menu, which stays open for its list.
  fn show(&self, page: Page, menu: &Signal<bool>) -> impl Fn() + Send + Sync + 'static {
    let (shown, open, menu) = (self.page.clone(), self.open.clone(), menu.clone());
    move || {
      shown.set(Some(page));
      open.set(true);
      menu.set(true);
    }
  }
}

/// The rows of `team`'s *More actions* menu, their ids starting with `id`, closed by a choice through `open`.
pub(in crate::screens::tasks) fn rows(
  ctx: &mut Ctx,
  (project, team): (ProjectId, &RowTeam),
  id: &str,
  (open, lists): (&Signal<bool>, &Lists),
) -> Vec<Element> {
  let task: TaskKey = team.key.parse().expect("a task's key reads back as a task key");
  let navigation = Navigation::of(ctx);
  let route = Route::Project(project, ProjectPage::Task(task, TaskView::default()));
  let open_task = MenuItem::new(format!("{id}-open"), ctx.t("tasks.board.menu.open"), move || {
    navigation.open(route.clone(), OpenIn::ActiveTab);
  })
  .icon(Icon::ArrowUpRight)
  .shortcut(ctx.t("tasks.board.keys.enter"));
  let copy = copy_key_item(ctx, format!("{id}-copy"), team.key.clone()).shortcut(ctx.t(if cfg!(target_os = "macos") {
    "tasks.board.keys.copyMac"
  } else {
    "tasks.board.keys.copy"
  }));
  let mut rows = vec![open_task.render(ctx, open)];
  let Some(offered) = &team.offered else {
    rows.push(copy.render(ctx, open));
    return rows;
  };
  let items = Items {
    id,
    team,
    offered,
    actions: TeamActions::of(ctx),
    lists,
  };
  let moves = items
    .moves(ctx, open)
    .map(|row| items.beside(Page::MoveTo, row.render(ctx, open)));
  rows.extend(moves);
  rows.push(copy.render(ctx, open));
  let changes = items.changes(ctx, open);
  let foot = foot(ctx, team, offered);
  if !changes.is_empty() || foot.is_some() {
    rows.push(separator(ctx));
  }
  rows.extend(changes);
  rows.extend(foot);
  rows.extend(items.list(ctx, open));
  rows
}

/// The items of one task's menu: each makes one lifecycle change of the task as the list read it.
struct Items<'a> {
  id: &'a str,
  team: &'a RowTeam,
  offered: &'a Offered,
  actions: Option<TeamActions>,
  lists: &'a Lists,
}

impl Items<'_> {
  fn closed(&self) -> bool {
    self.team.organization.status().is_closed()
  }

  fn ask(&self, change: Change) -> Ask {
    Ask {
      task: self.team.id,
      key: self.team.key.clone(),
      base: self.team.organization.clone(),
      revision: self.offered.revision,
      change,
    }
  }

  /// The item `suffix` that makes `change`.
  fn item(&self, label: Arc<str>, (suffix, glyph): (&str, Icon), change: Change) -> MenuItem {
    let (actions, ask) = (self.actions.clone(), self.ask(change));
    MenuItem::new(format!("{}-{suffix}", self.id), label, move || {
      if let Some(actions) = &actions {
        actions.run(ask.clone());
      }
    })
    .icon(glyph)
  }

  /// `row` of the list `page`, which that list opens beside while it is open.
  fn beside(&self, page: Page, row: Element) -> Element {
    if self.lists.page.get() != Some(page) {
      return row;
    }
    Column::new()
      .width(Dimension::Pct(100.0))
      .ref_element(self.lists.anchor.clone())
      .child(row)
      .into()
  }

  /// *Move to* for a task in Backlog or Todo, while the person may.
  fn moves(&self, ctx: &Ctx, open: &Signal<bool>) -> Option<MenuItem> {
    let status = self.team.organization.status();
    let movable = matches!(status, WorkflowStatus::Backlog | WorkflowStatus::Todo);
    (self.offered.abilities.triage && movable).then(|| {
      MenuItem::new(
        format!("{}-move", self.id),
        ctx.t("tasks.board.menu.moveTo"),
        self.lists.show(Page::MoveTo, open),
      )
      .icon(Icon::ArrowRightLeft)
      .shortcut(ctx.t("tasks.board.keys.move"))
      .submenu()
    })
  }

  /// Mark as done, Mark as blocked or Clear blocked and Cancel task for an open task, *Reopen task* for a closed one,
  /// those the person may make.
  fn changes(&self, ctx: &mut Ctx, open: &Signal<bool>) -> Vec<Element> {
    let abilities = self.offered.abilities;
    let mut changes = Vec::new();
    if self.closed() {
      if abilities.reopen {
        let reopen = MenuItem::new(
          format!("{}-reopen", self.id),
          ctx.t("tasks.team.lifecycle.reopenTask"),
          self.lists.show(Page::Reopen, open),
        )
        .icon(Icon::RotateCcw)
        .submenu();
        changes.push(self.beside(Page::Reopen, reopen.render(ctx, open)));
      }
      return changes;
    }
    let mut items = Vec::new();
    if abilities.complete {
      let done = self.item(
        ctx.t("tasks.team.lifecycle.done"),
        ("done", Icon::CircleCheck),
        Change::Done,
      );
      items.push(done);
    }
    if abilities.block {
      items.push(self.block(ctx));
    }
    if abilities.cancel {
      let cancel = self.item(
        ctx.t("tasks.team.lifecycle.cancel"),
        ("cancel", Icon::CircleX),
        Change::Cancel,
      );
      items.push(cancel);
    }
    changes.extend(items.into_iter().map(|item| item.render(ctx, open)));
    changes
  }

  /// *Clear blocked* for a blocked task; *Mark as blocked* otherwise, which asks for its reason first.
  fn block(&self, ctx: &Ctx) -> MenuItem {
    if self.team.organization.blocked_reason().is_some() {
      return self.item(
        ctx.t("tasks.team.lifecycle.clearBlocked"),
        ("clear-blocked", Icon::CirclePlay),
        Change::ClearBlocked,
      );
    }
    let (actions, ask) = (self.actions.clone(), self.ask(Change::Block(String::new())));
    MenuItem::new(
      format!("{}-block", self.id),
      ctx.t("tasks.team.lifecycle.block"),
      move || {
        if let Some(actions) = &actions {
          actions.ask_reason(ask.clone());
        }
      },
    )
    .icon(Icon::Ban)
  }

  /// The open list beside its row: Backlog and Todo for *Move to*, Back to Backlog and Back to Todo for *Reopen task*.
  fn list(&self, ctx: &mut Ctx, open: &Signal<bool>) -> Option<Element> {
    let page = self.lists.page.get()?;
    let (rows, foot) = match page {
      Page::MoveTo => (self.move_rows(ctx, open), "tasks.team.lifecycle.moveFoot"),
      Page::Reopen => (self.reopen_rows(ctx, open), "tasks.team.lifecycle.reopenFoot"),
    };
    let panel = menu_panel(&format!("{}-list", self.id))
      .width(Dimension::Px(LIST_WIDTH))
      .with_children(rows)
      .child(separator(ctx))
      .child(note(&format!("{}-list-foot", self.id), &ctx.t(foot)));
    // A press elsewhere in the menu closes the list and reaches the row it is on.
    let list = Popup::new(self.lists.anchor.clone(), panel)
      .open(self.lists.open.clone())
      .placement(Placement::RightStart)
      .offset(LIST_OFFSET.0, LIST_OFFSET.1)
      .outside_press(OutsidePress::PassThrough)
      .switch_on_anchor_press(false);
    Some(list.into())
  }

  /// Backlog and Todo, the task's own checked.
  fn move_rows(&self, ctx: &mut Ctx, open: &Signal<bool>) -> Vec<Element> {
    let now = self.team.organization.status();
    [
      ("move-backlog", WorkflowStatus::Backlog),
      ("move-todo", WorkflowStatus::Todo),
    ]
    .into_iter()
    .map(|(suffix, status)| {
      let label = ctx.t(workflow_key(status));
      let glyph = workflow_look(status).0;
      let item = if status == now {
        MenuItem::new(format!("{}-{suffix}", self.id), label, || {})
          .icon(glyph)
          .chosen(true)
      } else {
        self.item(label, (suffix, glyph), Change::MoveTo(status))
      };
      item.render(ctx, open)
    })
    .collect()
  }

  /// Back to Backlog and Back to Todo.
  fn reopen_rows(&self, ctx: &mut Ctx, open: &Signal<bool>) -> Vec<Element> {
    [
      (
        "reopen-backlog",
        "tasks.team.lifecycle.backToBacklog",
        WorkflowStatus::Backlog,
      ),
      ("reopen-todo", "tasks.team.lifecycle.backToTodo", WorkflowStatus::Todo),
    ]
    .into_iter()
    .map(|(suffix, label, status)| {
      let glyph = workflow_look(status).0;
      self
        .item(ctx.t(label), (suffix, glyph), Change::MoveTo(status))
        .render(ctx, open)
    })
    .collect()
  }
}

/// A foot line of a menu or its list.
fn note(id: &str, words: &str) -> Element {
  Text::new(words)
    .id(id.to_owned())
    .variant(ExtraTypography::Note.role())
    .color(PaletteColor::TextMuted)
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Tight.role())
        .bottom(ExtraSpacing::Snug.role())
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm),
    )
    .into()
}

/// The line under the menu's changes saying who does the rest: the claim's holder and the Admins close a task, Admins
/// alone cancel, reopen and move it. Told only where the server listed the task's lifecycle, since a rule it never
/// denied is not one to tell.
fn foot(ctx: &Ctx, team: &RowTeam, offered: &Offered) -> Option<Element> {
  let abilities = offered.abilities;
  if !abilities.listed {
    return None;
  }
  let closed = team.organization.status().is_closed();
  let admins = team.admins.as_ref();
  let words = if closed && !abilities.reopen {
    if admins.is_empty() {
      ctx.t("tasks.team.lifecycle.footClosedUnnamed")
    } else {
      ctx.t_args("tasks.team.lifecycle.footClosed", [("admins", admins)])
    }
  } else if !closed && !abilities.complete {
    match (&offered.holder, admins.is_empty()) {
      (Some(holder), false) => ctx.t_args(
        "tasks.team.lifecycle.footHolder",
        [("holder", holder.as_str()), ("admins", admins)],
      ),
      (Some(holder), true) => ctx.t_args("tasks.team.lifecycle.footHolderUnnamed", [("holder", holder.as_str())]),
      (None, false) => ctx.t_args("tasks.team.lifecycle.footFree", [("admins", admins)]),
      (None, true) => ctx.t("tasks.team.lifecycle.footFreeUnnamed"),
    }
  } else if !closed && !abilities.cancel && !abilities.triage {
    ctx.t("tasks.team.lifecycle.footAdmins")
  } else {
    return None;
  };
  Some(
    Text::new(&words)
      .id("tasks-team-menu-foot")
      .variant(TypographyStyle::Caption)
      .color(PaletteColor::TextMuted)
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
      .into(),
  )
}
