//! What the panel says about each waiting item (`Needs you item` `bAeM6`): its glyph, and where its run stands;
//! the projects the panel lists, under its title. What an item is, what it asks, its task, how long it waits
//! and its action are worded as the Overview words them (`orchester_workspace::tasks::attention`).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::time::Timestamp;
use orchester_workspace::{
  display,
  tasks::{
    attention::{NeedItem, NeedKind},
    needs_you::{WaitingItem, WaitingKind},
  },
};

use crate::ui::{icon::Icon, message::message_text};

/// What an item is: its glyph and word (`x4f2I`, `pdnV7`; `fEuoK`, `mYdJV`, `HcUnV` in the notes).
pub(super) fn kind(ctx: &Ctx, item: &NeedItem) -> (Icon, Arc<str>) {
  let glyph = match item.kind {
    NeedKind::Approval => Icon::ShieldAlert,
    NeedKind::Question | NeedKind::Input => Icon::MessageCircleQuestion,
    NeedKind::PlanReview => Icon::Map,
    NeedKind::Decision => Icon::CircleAlert,
    NeedKind::Interrupted => Icon::CirclePause,
    NeedKind::StoppedAtLimit => Icon::Gauge,
    NeedKind::ReadyToMerge => Icon::GitMerge,
    NeedKind::Memory => Icon::BookOpen,
  };
  (glyph, message_text(ctx, &item.kind.label()))
}

/// The item's action: *Review* and *Answer* open it in place; the others open its task, except *Review
/// records*, which is unavailable until memory review is built.
pub(super) fn action(ctx: &Ctx, item: &NeedItem) -> Arc<str> {
  message_text(ctx, &item.kind.action())
}

/// *waiting 6 min*, or with `clock` *waiting since 14:06 · 6 min*.
pub(super) fn waiting(ctx: &Ctx, item: &NeedItem, clock: bool) -> Arc<str> {
  let now = Timestamp::now();
  if clock {
    let time = message_text(ctx, &display::waited(now, item.since())).to_string();
    return ctx.t_args(
      "needs_you.waitingSince",
      [("clock", display::clock(item.since())), ("time", time)],
    );
  }
  message_text(ctx, &item.wait_line(now, |time| message_text(ctx, time).to_string()))
}

/// What the item asks: *Builder asks to run cargo add serde*, *Plan v2 from Planner waits for your decision*.
pub(super) fn title(ctx: &Ctx, item: &NeedItem) -> Arc<str> {
  message_text(ctx, &item.title)
}

/// *KON-42 · Export task list as JSON*; *Project memory* for the project's records.
pub(super) fn task(ctx: &Ctx, item: &NeedItem) -> Arc<str> {
  message_text(ctx, &item.context())
}

/// *Run 142 · 03 Implement · Builder*, or with a checkpoint for an interrupted run.
pub(super) fn place(ctx: &Ctx, item: &WaitingItem) -> Option<Arc<str>> {
  let run = item.run?.1.get().to_string();
  let Some(step) = &item.step else {
    return Some(ctx.t_args("needs_you.where.run", [("run", run)]));
  };
  let step = ctx
    .t_args(
      "needs_you.stepLabel",
      [
        ("number", format!("{:02}", step.number)),
        ("step", step.name.to_string()),
      ],
    )
    .to_string();
  let agent = match &item.kind {
    WaitingKind::Approval(shown) => Some(shown.approval.agent.clone()),
    WaitingKind::Choice(choice) => Some(choice.agent.to_string()),
    WaitingKind::Question(question) => question.as_ref().map(|question| question.agent.to_string()),
    WaitingKind::Plan(plan) => Some(plan.agent.to_string()),
    WaitingKind::Interrupted {
      checkpoint: Some(checkpoint),
    } => {
      return Some(ctx.t_args(
        "needs_you.where.checkpoint",
        [("run", run), ("step", step), ("checkpoint", checkpoint.to_string())],
      ));
    }
    WaitingKind::Decision(_)
    | WaitingKind::Interrupted { .. }
    | WaitingKind::StoppedAtLimit { .. }
    | WaitingKind::ReadyToMerge
    | WaitingKind::Memory { .. } => None,
  };
  Some(match agent {
    Some(agent) => ctx.t_args(
      "needs_you.where.agent",
      [("run", run), ("step", step), ("agent", agent)],
    ),
    None => ctx.t_args("needs_you.where.step", [("run", run), ("step", step)]),
  })
}

/// *Kontur and Atlas · oldest first in each project*; with nothing waiting the projects alone.
pub(super) fn sub(ctx: &Ctx, names: &[Arc<str>], waiting: bool) -> Arc<str> {
  let projects = projects(ctx, names);
  let key = match (waiting, names.len()) {
    (false, _) => "needs_you.sub.none",
    (true, 1) => "needs_you.sub.one",
    (true, _) => "needs_you.sub.each",
  };
  ctx.t_args(key, [("projects", projects)])
}

/// *Kontur*, *Kontur and Atlas*, *Kontur, Atlas and Docs site*.
fn projects(ctx: &Ctx, names: &[Arc<str>]) -> String {
  match names {
    [] => String::new(),
    [one] => one.to_string(),
    [first, second] => ctx
      .t_args(
        "needs_you.projects.two",
        [("first", first.to_string()), ("second", second.to_string())],
      )
      .to_string(),
    [rest @ .., last] => {
      let separator = ctx.t("needs_you.projects.separator");
      let list = rest
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>()
        .join(&separator);
      ctx
        .t_args("needs_you.projects.many", [("list", list), ("last", last.to_string())])
        .to_string()
    }
  }
}
