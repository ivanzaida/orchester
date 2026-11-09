//! A team task's row menu on the Tasks list (decision D4, `Task - v2` `jDN0r` T7 `lCoNt`, T8 `vdSWS`, T10 `tlw0r`):
//! Open and Copy key, then only what the task's capabilities let the person do — Mark as done, Mark as blocked or Clear
//! blocked, Cancel task, Reopen, and a move between Backlog and Todo — and a foot line saying who does the rest. An
//! item the person may not use is absent, never disabled.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::Text,
  core::Signal,
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
  ui::{
    icon::Icon,
    menu::{MenuItem, separator},
  },
};

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

/// The rows of `team`'s *More actions* menu, their ids starting with `id`, closed by a choice through `open`.
pub(in crate::screens::tasks) fn rows(
  ctx: &mut Ctx,
  (project, team): (ProjectId, &RowTeam),
  id: &str,
  open: &Signal<bool>,
) -> Vec<Element> {
  let task: TaskKey = team.key.parse().expect("a task's key reads back as a task key");
  let navigation = Navigation::of(ctx);
  let route = Route::Project(project, ProjectPage::Task(task, TaskView::default()));
  let open_task = MenuItem::new(format!("{id}-open"), ctx.t("tasks.board.menu.open"), move || {
    navigation.open(route.clone(), OpenIn::ActiveTab);
  })
  .icon(Icon::ArrowUpRight);
  let copy = copy_key_item(ctx, format!("{id}-copy"), team.key.clone());
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
  };
  if let Some(moves) = items.moves(ctx) {
    rows.push(moves.render(ctx, open));
  }
  rows.push(copy.render(ctx, open));
  let changes = items.changes(ctx);
  let foot = foot(ctx, team, offered);
  if !changes.is_empty() || foot.is_some() {
    rows.push(separator(ctx));
  }
  rows.extend(changes.into_iter().map(|row| row.render(ctx, open)));
  rows.extend(foot);
  rows
}

/// The items of one task's menu: each makes one lifecycle change of the task as the list read it.
struct Items<'a> {
  id: &'a str,
  team: &'a RowTeam,
  offered: &'a Offered,
  actions: Option<TeamActions>,
}

impl Items<'_> {
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
  fn item(&self, ctx: &Ctx, suffix: &str, (label, glyph): (&str, Icon), change: Change) -> MenuItem {
    let (actions, ask) = (self.actions.clone(), self.ask(change));
    MenuItem::new(format!("{}-{suffix}", self.id), ctx.t(label), move || {
      if let Some(actions) = &actions {
        actions.run(ask.clone());
      }
    })
    .icon(glyph)
  }

  /// *Move to Todo* for a task in Backlog and *Move to Backlog* for any other open one, while the person may.
  fn moves(&self, ctx: &Ctx) -> Option<MenuItem> {
    let (suffix, label, to) = match self.team.organization.status() {
      WorkflowStatus::Backlog => ("move-todo", "tasks.team.lifecycle.moveTodo", WorkflowStatus::Todo),
      _ => (
        "move-backlog",
        "tasks.team.lifecycle.moveBacklog",
        WorkflowStatus::Backlog,
      ),
    };
    let moved = self.item(ctx, suffix, (label, Icon::ArrowRightLeft), Change::MoveTo(to));
    self.offered.abilities.triage.then_some(moved)
  }

  /// Mark as done, Mark as blocked or Clear blocked, Cancel task and Reopen, those the person may make.
  fn changes(&self, ctx: &Ctx) -> Vec<MenuItem> {
    let abilities = self.offered.abilities;
    let mut changes = Vec::new();
    if abilities.complete {
      let done = ("tasks.team.lifecycle.done", Icon::CircleCheck);
      changes.push(self.item(ctx, "done", done, Change::Done));
    }
    if abilities.block {
      changes.push(self.block(ctx));
    }
    if abilities.cancel {
      let cancel = ("tasks.team.lifecycle.cancel", Icon::CircleX);
      changes.push(self.item(ctx, "cancel", cancel, Change::Cancel));
    }
    if abilities.reopen {
      for (suffix, label, to) in [
        ("reopen-todo", "tasks.team.lifecycle.reopenTodo", WorkflowStatus::Todo),
        (
          "reopen-backlog",
          "tasks.team.lifecycle.reopenBacklog",
          WorkflowStatus::Backlog,
        ),
      ] {
        changes.push(self.item(ctx, suffix, (label, Icon::RotateCcw), Change::MoveTo(to)));
      }
    }
    changes
  }

  /// *Clear blocked* for a blocked task; *Mark as blocked* otherwise, which asks for its reason first.
  fn block(&self, ctx: &Ctx) -> MenuItem {
    if self.team.organization.blocked_reason().is_some() {
      let clear = ("tasks.team.lifecycle.clearBlocked", Icon::CirclePlay);
      return self.item(ctx, "clear-blocked", clear, Change::ClearBlocked);
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
}

/// The line under the menu's changes saying who does the rest: the claim's holder and the Admins close a task, Admins
/// alone cancel, reopen and move it.
fn foot(ctx: &Ctx, team: &RowTeam, offered: &Offered) -> Option<Element> {
  let abilities = offered.abilities;
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
