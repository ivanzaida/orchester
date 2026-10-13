//! What the panel says about each waiting item (`Needs you item` `bAeM6`): what it is, what it asks, its task
//! and where its run stands; the projects the panel lists, under its title.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::time::Timestamp;
use orchester_workspace::{
  display,
  tasks::needs_you::{WaitingItem, WaitingKind},
};

use crate::ui::{icon::Icon, message::message_text};

/// What an item is: its glyph and word (`x4f2I`, `pdnV7`; `fEuoK`, `mYdJV`, `HcUnV` in the notes).
pub(super) fn kind(ctx: &Ctx, kind: &WaitingKind) -> (Icon, Arc<str>) {
  let (glyph, key) = match kind {
    WaitingKind::Approval(_) => (Icon::ShieldAlert, "needs_you.kind.approval"),
    WaitingKind::Choice(_) | WaitingKind::Question(_) => (Icon::MessageCircleQuestion, "needs_you.kind.question"),
    WaitingKind::Plan(_) => (Icon::Map, "needs_you.kind.plan"),
    WaitingKind::Decision(_) => (Icon::CircleAlert, "needs_you.kind.decision"),
    WaitingKind::Interrupted { .. } => (Icon::CirclePause, "needs_you.kind.interrupted"),
    WaitingKind::StoppedAtLimit { .. } => (Icon::Gauge, "needs_you.kind.limit"),
    WaitingKind::ReadyToMerge => (Icon::GitMerge, "needs_you.kind.merge"),
    WaitingKind::Memory { .. } => (Icon::BookOpen, "needs_you.kind.memory"),
  };
  (glyph, ctx.t(key))
}

/// The item's action: *Review* and *Answer* open it in place; the others open its task, or the project's.
pub(super) fn action(ctx: &Ctx, kind: &WaitingKind) -> Arc<str> {
  ctx.t(match kind {
    WaitingKind::Approval(_) => "needs_you.action.review",
    WaitingKind::Choice(_) | WaitingKind::Question(_) => "needs_you.action.answer",
    WaitingKind::Plan(_) => "needs_you.action.reviewPlan",
    WaitingKind::Decision(_) => "needs_you.action.decide",
    WaitingKind::Interrupted { .. } => "needs_you.action.open",
    WaitingKind::StoppedAtLimit { .. } => "needs_you.action.raiseLimit",
    WaitingKind::ReadyToMerge => "needs_you.action.merge",
    WaitingKind::Memory { .. } => "needs_you.action.memory",
  })
}

/// *waiting 6 min*, or with `clock` *waiting since 14:06 · 6 min*.
pub(super) fn waiting(ctx: &Ctx, since: Timestamp, clock: bool) -> Arc<str> {
  let time = message_text(ctx, &display::waited(Timestamp::now(), since)).to_string();
  if clock {
    ctx.t_args(
      "needs_you.waitingSince",
      [("clock", display::clock(since)), ("time", time)],
    )
  } else {
    ctx.t_args("needs_you.waiting", [("time", time)])
  }
}

/// What the item asks: *Builder asks to run cargo add serde*, *Plan v2 from Planner waits for your decision*.
pub(super) fn title(ctx: &Ctx, item: &WaitingItem) -> Arc<str> {
  match &item.kind {
    WaitingKind::Approval(shown) => {
      let approval = &shown.approval;
      if !approval.command {
        return ctx.t_args(
          "needs_you.asks.tool",
          [
            ("agent", approval.agent.as_str()),
            ("server", approval.server.as_str()),
            ("tool", approval.action.as_str()),
          ],
        );
      }
      let command = match shown.first_line() {
        (line, true) => ctx.t_args("needs_you.asks.firstLineMore", [("line", line)]).to_string(),
        (line, false) => line.to_owned(),
      };
      ctx.t_args(
        "needs_you.asks.command",
        [("agent", approval.agent.clone()), ("command", command)],
      )
    }
    WaitingKind::Choice(choice) if choice.count > 1 => ctx.t_args(
      "needs_you.asks.questions",
      [("agent", choice.agent.to_string()), ("count", choice.count.to_string())],
    ),
    WaitingKind::Choice(choice) => asks(ctx, &choice.agent, &choice.first),
    WaitingKind::Question(Some(question)) => asks(ctx, &question.agent, &question.text),
    WaitingKind::Question(None) => ctx.t("needs_you.asks.unread"),
    WaitingKind::Plan(plan) => ctx.t_args(
      "needs_you.plan",
      [("version", plan.version.to_string()), ("agent", plan.agent.to_string())],
    ),
    WaitingKind::Decision(decision) => message_text(ctx, &decision.title()),
    WaitingKind::Interrupted { .. } => of_run(ctx, "needs_you.interrupted", item),
    WaitingKind::StoppedAtLimit { daily_budget: true } => of_run(ctx, "needs_you.stoppedDaily", item),
    WaitingKind::StoppedAtLimit { daily_budget: false } => of_run(ctx, "needs_you.stoppedAtLimit", item),
    WaitingKind::ReadyToMerge => of_run(ctx, "needs_you.readyToMerge", item),
    WaitingKind::Memory { count: 1 } => ctx.t("needs_you.memory.one"),
    WaitingKind::Memory { count } => ctx.t_args("needs_you.memory.other", [("count", count.to_string())]),
  }
}

/// *Run 142 was interrupted*: `key`'s words about the item's run.
fn of_run(ctx: &Ctx, key: &str, item: &WaitingItem) -> Arc<str> {
  ctx.t_args(
    key,
    [(
      "run",
      item.run.map(|(_, number)| number.get().to_string()).unwrap_or_default(),
    )],
  )
}

/// *Explorer asks: index the vendored crates?*, the question's first line.
fn asks(ctx: &Ctx, agent: &str, question: &str) -> Arc<str> {
  let first = question
    .lines()
    .find(|line| !line.trim().is_empty())
    .unwrap_or_default();
  ctx.t_args(
    "needs_you.asks.question",
    [("agent", agent), ("question", first.trim())],
  )
}

/// *KON-42 · Export task list as JSON*; *Project memory* for the project's records.
pub(super) fn task(ctx: &Ctx, item: &WaitingItem) -> Arc<str> {
  match (&item.task, &item.kind) {
    (Some(task), _) => ctx.t_args(
      "needs_you.task",
      [("key", task.key.to_string()), ("title", task.title.to_string())],
    ),
    (None, WaitingKind::Memory { .. }) => ctx.t("needs_you.projectMemory"),
    (None, _) => ctx.t("needs_you.noTask"),
  }
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
