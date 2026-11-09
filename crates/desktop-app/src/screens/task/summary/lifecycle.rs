//! A team task's lifecycle in the Summary's claim area (decision D4, `Task - v2` `jDN0r` rows 14–17: T1–T4 `rvvoN`
//! `o4K8uq` `g9S4k` `V26A3`, the blocked states T11 `VVhcE`): the actions each state calls for and who may do the
//! rest. While the task is open, *Mark as done*, *Cancel task*, and *Mark as blocked* or *Clear blocked* sit under the
//! claim strip, each only while the task's capabilities say the person may (`TeamTask::abilities`), never as a
//! disabled button; the first is the primary action and the others ghost buttons. A blocked task is one card (its
//! badge, its reason, what it means, the actions). A person without an action reads one lock line under them, naming
//! who can; it is told only where the server's capabilities list the lifecycle at all. A Done or Canceled task is a
//! card as well, with *Reopen* under its words for an Admin. The app knows only who changed the task last, not who
//! closed or blocked it, so no card names a person or a time.

use lurq::{
  app::ctx::Ctx,
  components::Row,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::task::organization::WorkflowStatus;

use crate::{
  screens::task::{
    TaskContext,
    team_controls::{TeamAsk, TeamControls, reopen_menu},
  },
  team::{
    lock,
    pages::{Lifecycle, TeamTask},
    state_card::StateCard,
  },
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    chip::BadgeTone,
    icon::Icon,
  },
};

/// Who the lock line says does the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
  /// The person closes or blocks the task but does not cancel it: only Admins cancel or reopen.
  Holder,
  /// A Done or Canceled task the person cannot reopen: only Admins do.
  Closed,
  /// The person may do none of it: the claim's holder and the Admins close the task, or clear its block.
  Open { blocked: bool },
}

/// The rule the Summary states for `task`, once the server's capabilities list the lifecycle and the person lacks some
/// of it; `None` for an Admin, who has all of it, and for a task whose capabilities list none.
fn rule(task: &TeamTask) -> Option<Rule> {
  let abilities = task.abilities.lifecycle;
  if !abilities.listed {
    return None;
  }
  let status = task.task.organization().status();
  if status.is_closed() {
    return (!abilities.reopen).then_some(Rule::Closed);
  }
  let acts = abilities.complete || abilities.cancel || abilities.block;
  match (acts, abilities.cancel) {
    (true, true) => None,
    (true, false) => Some(Rule::Holder),
    (false, _) => Some(Rule::Open {
      blocked: task.task.organization().blocked_reason().is_some(),
    }),
  }
}

/// Whether the claim area says who does the rest, so the page's own read-only line steps aside.
pub fn states_rule(task: &TaskContext) -> bool {
  task.team.as_ref().is_some_and(|team| rule(&team.task).is_some())
}

/// The one lock line of `task`'s state, naming who can do what the person cannot.
fn lock_line(ctx: &Ctx, task: &TaskContext) -> Option<Element> {
  let team = task.team.as_ref()?;
  let rule = rule(&team.task)?;
  let admins = team.standing.admins.as_deref();
  let holder = team.task.claim.as_ref().map(|claim| claim.holder.name.clone());
  let words = match rule {
    Rule::Holder => lock::lock_words(
      ctx,
      admins,
      (
        "summary.team.lifecycle.lockHolder",
        "summary.team.lifecycle.lockHolderUnnamed",
      ),
    ),
    Rule::Closed => lock::lock_words(
      ctx,
      admins,
      (
        "summary.team.lifecycle.lockClosed",
        "summary.team.lifecycle.lockClosedUnnamed",
      ),
    ),
    Rule::Open { blocked } => {
      let names = admins.map(|admins| lock::plain_names(ctx, admins));
      let names = names.filter(|names| !names.is_empty());
      let (named, unnamed) = match (holder.is_some(), blocked) {
        (true, true) => (
          "summary.team.lifecycle.lockClear",
          "summary.team.lifecycle.lockClearUnnamed",
        ),
        (true, false) => (
          "summary.team.lifecycle.lockOpen",
          "summary.team.lifecycle.lockOpenUnnamed",
        ),
        (false, _) => (
          "summary.team.lifecycle.lockOpenFree",
          "summary.team.lifecycle.lockOpenFreeUnnamed",
        ),
      };
      let holder = holder.unwrap_or_default();
      match names {
        Some(names) => ctx.t_args(named, [("holder", holder), ("admins", names)]).to_string(),
        None => ctx.t_args(unnamed, [("holder", holder)]).to_string(),
      }
    }
  };
  Some(lock::locked("task-team-lifecycle-lock", &words).into())
}

/// The card of a Done or Canceled task: what became of it and that its claim ended with it, with *Reopen* under the
/// words for an Admin and, for a person who cannot reopen it, the lock line. `None` for an open task.
pub fn closed(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let team = task.team.as_ref()?;
  let status = team.task.task.organization().status();
  if !status.is_closed() {
    return None;
  }
  let done = status == WorkflowStatus::Done;
  let (glyph, title, detail) = if done {
    (
      Icon::CircleCheck,
      ctx.t("summary.team.lifecycle.doneTitle"),
      ctx.t("summary.team.lifecycle.doneDetail"),
    )
  } else {
    (
      Icon::CircleSlash,
      ctx.t("summary.team.lifecycle.canceledTitle"),
      ctx.t("summary.team.lifecycle.canceledDetail"),
    )
  };
  let controls = TeamControls::of(ctx).filter(|_| team.task.abilities.lifecycle.reopen);
  let reopen = controls.map(|controls| reopen_button(ctx, &controls));
  let lock = lock_line(ctx, task);
  Some(
    StateCard::new(
      "task-team-closed",
      (BadgeTone::Neutral, glyph, title.clone()),
      title,
      detail,
    )
    .actions(reopen)
    .lock(lock)
    .render(),
  )
}

/// *Reopen*, with its two choices under it while they are open.
fn reopen_button(ctx: &mut Ctx, controls: &TeamControls) -> Element {
  let open = controls.reopening.clone();
  let button = ActionButton::new(
    "task-reopen",
    ButtonKind::Secondary,
    ctx.t("summary.team.lifecycle.reopen.label"),
  )
  .icon(Icon::RotateCcw)
  .pinned();
  let button = if controls.pending() {
    button.loading()
  } else {
    button.on_click(move || open.update(|open| *open = !*open))
  };
  let menu = controls
    .reopening
    .get()
    .then(|| reopen_menu(ctx, &controls.reopen_anchor))
    .flatten();
  Row::new()
    .ref_element(controls.reopen_anchor.clone())
    .child(button)
    .with_children(menu)
    .into()
}

/// What an open task adds under its claim strip: a blocked task's card with the actions in it, or the actions the
/// person may take and the one lock line, or only that line.
pub fn parts(ctx: &mut Ctx, task: &TaskContext) -> Vec<Element> {
  let Some(team) = task.team.as_ref() else {
    return Vec::new();
  };
  if team.task.task.organization().status().is_closed() {
    return Vec::new();
  }
  let actions = actions(ctx, &team.task);
  let lock = lock_line(ctx, task);
  match blocked_card(ctx, task, (actions, lock)) {
    Ok(card) => vec![card],
    Err((actions, lock)) => actions.into_iter().chain(lock).collect(),
  }
}

/// The card of a blocked task, or the actions and the lock line given when the task is not blocked.
fn blocked_card(
  ctx: &Ctx,
  task: &TaskContext,
  (actions, lock): (Option<Element>, Option<Element>),
) -> Result<Element, (Option<Element>, Option<Element>)> {
  let Some(team) = task.team.as_ref() else {
    return Err((actions, lock));
  };
  let Some(reason) = team.task.task.organization().blocked_reason() else {
    return Err((actions, lock));
  };
  let abilities = team.task.abilities.lifecycle;
  let detail = match (abilities.block, team.task.claim.as_ref()) {
    (false, _) => ctx.t("summary.team.lifecycle.blockedPlain"),
    (true, Some(claim)) if claim.holder.you => ctx.t("summary.team.lifecycle.blockedHolder"),
    (true, Some(claim)) => ctx.t_args(
      "summary.team.lifecycle.blockedAdmin",
      [("holder", claim.holder.name.as_str())],
    ),
    (true, None) => ctx.t("summary.team.lifecycle.blockedFree"),
  };
  let title = ctx.t("summary.team.lifecycle.blockedTitle");
  Ok(
    StateCard::new(
      "task-team-blocked",
      (BadgeTone::Warning, Icon::Ban, title.clone()),
      title,
      detail,
    )
    .reason(reason.to_owned())
    .actions(actions)
    .lock(lock)
    .render(),
  )
}

/// What pressing a lifecycle button does.
enum Press {
  Ask(TeamAsk),
  /// Opens the question for Mark as blocked's reason.
  Reason,
}

/// The actions of an open task the person may take, in the order the design draws them: a blocked task's *Clear
/// blocked* first, *Mark as done*, *Cancel task*, and *Mark as blocked* last.
fn wanted(abilities: Lifecycle, blocked: bool) -> Vec<((&'static str, &'static str, Icon), Press)> {
  let clear = (
    (
      "task-team-clear-blocked",
      "summary.team.lifecycle.clearBlocked",
      Icon::CirclePlay,
    ),
    Press::Ask(TeamAsk::ClearBlocked),
  );
  let block = (
    ("task-team-block", "summary.team.lifecycle.block.label", Icon::Ban),
    Press::Reason,
  );
  let mut wanted = Vec::new();
  if abilities.block && blocked {
    wanted.push(clear);
  }
  if abilities.complete {
    wanted.push((
      ("task-team-done", "summary.team.lifecycle.done", Icon::CircleCheck),
      Press::Ask(TeamAsk::Done),
    ));
  }
  if abilities.cancel {
    wanted.push((
      ("task-team-cancel", "summary.team.lifecycle.cancel", Icon::CircleX),
      Press::Ask(TeamAsk::Cancel),
    ));
  }
  if abilities.block && !blocked {
    wanted.push(block);
  }
  wanted
}

/// *Mark as done*, *Cancel task* and *Mark as blocked* or *Clear blocked*, those the person may take: the first a
/// primary button, the others ghost; `None` when none.
fn actions(ctx: &mut Ctx, task: &TeamTask) -> Option<Element> {
  let controls = TeamControls::of(ctx)?;
  let blocked = task.task.organization().blocked_reason().is_some();
  let wanted = wanted(task.abilities.lifecycle, blocked);
  if wanted.is_empty() {
    return None;
  }
  let buttons: Vec<Element> = wanted
    .into_iter()
    .enumerate()
    .map(|(index, ((id, label, glyph), press))| {
      let kind = if index == 0 {
        ButtonKind::Primary
      } else {
        ButtonKind::Ghost
      };
      let button = ActionButton::new(id, kind, ctx.t(label)).icon(glyph).pinned();
      if controls.pending() {
        return button.loading().into();
      }
      let controls = controls.clone();
      match press {
        Press::Ask(ask) => button.on_click(move || controls.run(ask)),
        Press::Reason => button.on_click(move || {
          controls.reason.set(String::new());
          controls.blocking.set(true);
        }),
      }
      .into()
    })
    .collect();
  Some(
    Row::new()
      .id("task-team-lifecycle")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .with_children(buttons)
      .into(),
  )
}
