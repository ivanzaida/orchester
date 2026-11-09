//! A team task's lifecycle in the Summary's claim area (decision D4, `Task - v2` `jDN0r` rows 14–17: T1–T4 `rvvoN`
//! `o4K8uq` `g9S4k` `V26A3`, the blocked states T11 `VVhcE`): the one action each state calls for and who may do the
//! rest. *Mark as done*, *Cancel task*, *Mark as blocked* or *Clear blocked* sit under the claim strip while the task
//! is open, each only while the task's capabilities say the person may (`TeamTask::abilities`), never as a disabled
//! button: a member without the action reads one lock line naming who can. A Done or Canceled task's strip says who
//! closed it and when and that closing ended its claim, with *Reopen* for an Admin. The claim strip itself stays
//! [`super::claim`]'s while the task is open.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::task::organization::WorkflowStatus;

use crate::{
  screens::{
    settings::orchester::moment,
    task::{
      TaskContext,
      team_controls::{TeamAsk, TeamControls, reopen_menu},
    },
  },
  team::{
    lock,
    pages::{TeamPerson, TeamTask},
    strip::{Strip, StripTone},
  },
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// The strip of a Done or Canceled task: who closed it and when, and that closing ended its claim; *Reopen* for an
/// Admin. `None` for an open task.
pub fn closed(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let team = task.team.as_ref()?;
  let status = team.task.task.organization().status();
  if !status.is_closed() {
    return None;
  }
  let who = person_words(ctx, &team.task.updated.by);
  let time = moment(ctx, team.task.updated.at);
  let arguments = [("who", who.as_str()), ("time", time.as_str())];
  let (glyph, tone, title) = if status == WorkflowStatus::Done {
    (
      (Icon::CircleCheck, PaletteColor::Success),
      StripTone::Success,
      ctx.t_args("summary.team.lifecycle.doneTitle", arguments),
    )
  } else {
    (
      (Icon::CircleSlash, PaletteColor::TextSecondary),
      StripTone::Plain,
      ctx.t_args("summary.team.lifecycle.canceledTitle", arguments),
    )
  };
  let strip = Strip::new(
    "task-team-closed",
    glyph,
    title,
    ctx.t("summary.team.lifecycle.closedDetail"),
  )
  .tone(tone);
  let controls = TeamControls::of(ctx).filter(|_| team.task.abilities.lifecycle.reopen);
  Some(match controls {
    Some(controls) => strip.action(reopen_button(ctx, &controls)).into(),
    None => strip.into(),
  })
}

/// *Reopen*, with its two choices beside it while they are open.
fn reopen_button(ctx: &mut Ctx, controls: &TeamControls) -> Element {
  let open = controls.reopening.clone();
  let button = ActionButton::new(
    "task-reopen",
    ButtonKind::Secondary,
    ctx.t("summary.team.lifecycle.reopen.label"),
  )
  .icon(Icon::RotateCcw)
  .small();
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

/// What a task adds under its claim strip: while open, the blocked reason when it has one, then the actions the person
/// may take, or the one lock line naming who can; while closed, the lock line of a person who cannot reopen it.
pub fn parts(ctx: &mut Ctx, task: &TaskContext) -> Vec<Element> {
  let Some(team) = task.team.as_ref() else {
    return Vec::new();
  };
  if team.task.task.organization().status().is_closed() {
    return closed_lock(ctx, task).into_iter().collect();
  }
  let blocked = blocked_strip(ctx, &team.task);
  let actions = actions(ctx, &team.task);
  let lock = actions.is_none().then(|| open_lock(ctx, task)).flatten();
  blocked.into_iter().chain(actions).chain(lock).collect()
}

/// A closed task for a person who cannot reopen it: only Admins do.
fn closed_lock(ctx: &Ctx, task: &TaskContext) -> Option<Element> {
  let team = task
    .team
    .as_ref()
    .filter(|team| !team.task.abilities.lifecycle.reopen)?;
  let words = lock::lock_words(
    ctx,
    team.standing.admins.as_deref(),
    (
      "summary.team.lifecycle.lockClosed",
      "summary.team.lifecycle.lockClosedUnnamed",
    ),
  );
  Some(lock::locked("task-team-lifecycle-lock", &words).into())
}

/// An open task whose lifecycle the person may not touch: who closes it, the claim's holder and the Admins.
fn open_lock(ctx: &Ctx, task: &TaskContext) -> Option<Element> {
  let team = task.team.as_ref()?;
  let admins = team
    .standing
    .admins
    .as_deref()
    .map(|admins| lock::admin_names(ctx, admins))
    .filter(|names| !names.is_empty());
  let words = match (&team.task.claim, admins) {
    (Some(claim), Some(admins)) => ctx.t_args(
      "summary.team.lifecycle.lockOpen",
      [("holder", claim.holder.name.clone()), ("admins", admins)],
    ),
    (Some(claim), None) => ctx.t_args(
      "summary.team.lifecycle.lockOpenUnnamed",
      [("holder", claim.holder.name.clone())],
    ),
    (None, Some(admins)) => ctx.t_args("summary.team.lifecycle.lockOpenFree", [("admins", admins)]),
    (None, None) => ctx.t("summary.team.lifecycle.lockOpenFreeUnnamed"),
  };
  Some(lock::locked("task-team-lifecycle-lock", &words).into())
}

/// The blocked reason, with who changed the task last (who set it, unless a later change replaced that) and when.
fn blocked_strip(ctx: &Ctx, task: &TeamTask) -> Option<Element> {
  let reason = task.task.organization().blocked_reason()?;
  let who = person_words(ctx, &task.updated.by);
  let time = moment(ctx, task.updated.at);
  let title = ctx.t_args(
    "summary.team.lifecycle.blockedTitle",
    [("who", who.as_str()), ("time", time.as_str())],
  );
  Some(
    Strip::new(
      "task-team-blocked",
      (Icon::Ban, PaletteColor::Warning),
      title,
      reason.to_owned(),
    )
    .tone(StripTone::Warning)
    .into(),
  )
}

/// *Mark as done*, *Cancel task* and *Mark as blocked* or *Clear blocked*, those the person may take; `None` when none.
fn actions(ctx: &mut Ctx, task: &TeamTask) -> Option<Element> {
  let controls = TeamControls::of(ctx)?;
  let lifecycle = task.abilities.lifecycle;
  let blocked = task.task.organization().blocked_reason().is_some();
  let asking = |ask: TeamAsk| {
    let controls = controls.clone();
    move || controls.run(ask)
  };
  let mut wanted = Vec::new();
  if lifecycle.complete {
    wanted.push((
      ("task-team-done", "summary.team.lifecycle.done", Icon::CircleCheck),
      Press::Ask(TeamAsk::Done),
    ));
  }
  if lifecycle.cancel {
    wanted.push((
      ("task-team-cancel", "summary.team.lifecycle.cancel", Icon::CircleX),
      Press::Ask(TeamAsk::Cancel),
    ));
  }
  if lifecycle.block {
    wanted.push(if blocked {
      (
        (
          "task-team-clear-blocked",
          "summary.team.lifecycle.clearBlocked",
          Icon::CirclePlay,
        ),
        Press::Ask(TeamAsk::ClearBlocked),
      )
    } else {
      (
        ("task-team-block", "summary.team.lifecycle.block.label", Icon::Ban),
        Press::Reason,
      )
    });
  }
  if wanted.is_empty() {
    return None;
  }
  let buttons: Vec<Element> = wanted
    .into_iter()
    .map(|((id, label, glyph), press)| {
      let button = ActionButton::new(id, ButtonKind::Secondary, ctx.t(label))
        .icon(glyph)
        .small();
      let button = if controls.pending() {
        button.loading()
      } else {
        match press {
          Press::Ask(ask) => button.on_click(asking(ask)),
          Press::Reason => {
            let (open, reason) = (controls.blocking.clone(), controls.reason.clone());
            button.on_click(move || {
              reason.set(String::new());
              open.set(true);
            })
          }
        }
      };
      button.into()
    })
    .collect();
  Some(
    Row::new()
      .id("task-team-lifecycle")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .with_children(buttons)
      .into(),
  )
}

/// What pressing a lifecycle button does.
enum Press {
  Ask(TeamAsk),
  /// Opens the question for Mark as blocked's reason.
  Reason,
}

/// `you` for the person signed in, else the name the server gives.
fn person_words(ctx: &Ctx, person: &TeamPerson) -> String {
  if person.you {
    ctx.t("summary.team.lifecycle.you").to_string()
  } else {
    person.name.clone()
  }
}
