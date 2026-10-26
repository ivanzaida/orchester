//! A row's status icon and its one state phrase (`Status` and `State` in `KON-12` `r0eIJ`), both in the tone it calls
//! for. A task something of waits for the person says what, its oldest wait first, in `warning` (Approve
//! `cargo add rand`, Answer a question, Review plan v1, Stopped at its token limit), or in `success` for work that waits
//! to be merged or completed. The others say where they stand in the workspace's words for them (`row.detail`): a run
//! at work by its step (`02 Plan`) in `text_secondary`, a failed one at which step in `danger`, one that cannot be read
//! in `danger`, what waits in the queue or is ready to start in `text_muted`, what is held as its group holds it.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{run::TaskPhase, task_group::GroupHold};
use orchester_workspace::tasks::{
  attention::{NeedItem, NeedKind},
  data::ProjectTasks,
  list::{CurrentStep, ReadinessKind, TaskRow},
  needs_you::WaitingKind,
};

use crate::ui::{icon::Icon, message::message_text};

/// What a row says about where its task stands.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Phrase {
  pub glyph: Icon,
  pub glyph_color: PaletteColor,
  pub words: Arc<str>,
  pub color: PaletteColor,
}

impl Phrase {
  /// What `row` says, `need` being the oldest thing its task waits for from the person, if any, among `tasks`.
  pub(super) fn of(ctx: &Ctx, row: &TaskRow, need: Option<&NeedItem>, tasks: &ProjectTasks) -> Self {
    match need {
      Some(need) => waits(ctx, need, tasks),
      None => stands(ctx, row),
    }
  }
}

/// What a task waits for from the person.
fn waits(ctx: &Ctx, need: &NeedItem, tasks: &ProjectTasks) -> Phrase {
  let (glyph, words) = match (&need.waiting.kind, need.kind) {
    (WaitingKind::Approval(shown), _) if shown.approval.command => {
      let (line, _) = shown.first_line();
      (Icon::Hand, ctx.t_args("tasks.state.approveCommand", [("command", line)]))
    }
    (WaitingKind::Approval(shown), _) => (
      Icon::Hand,
      ctx.t_args("tasks.state.approveTool", [("tool", shown.approval.action.to_string())]),
    ),
    (WaitingKind::Choice(choice), _) if choice.count > 1 => (
      Icon::MessageCircle,
      ctx.t_args("tasks.state.answerMany", [("count", choice.count.to_string())]),
    ),
    (WaitingKind::Plan(plan), _) => (
      Icon::Map,
      ctx.t_args("tasks.state.reviewPlan", [("version", plan.version.to_string())]),
    ),
    (WaitingKind::StoppedAtLimit { daily_budget }, _) => (Icon::Gauge, ctx.t(limit_key(need, tasks, *daily_budget))),
    (_, kind) => (kind_glyph(kind), ctx.t(kind_key(kind))),
  };
  let color = match need.kind {
    NeedKind::ReadyToMerge | NeedKind::ReadyToComplete => PaletteColor::Success,
    _ => PaletteColor::Warning,
  };
  Phrase {
    glyph,
    glyph_color: color.clone(),
    words,
    color,
  }
}

/// The words of a run stopped at a limit: the daily budget, its own token limit, or another of its limits.
fn limit_key(need: &NeedItem, tasks: &ProjectTasks, daily_budget: bool) -> &'static str {
  let tokens = need
    .task()
    .and_then(|task| tasks.phases.get(&task.id)?.run()?.token_limit)
    .is_some();
  match (daily_budget, tokens) {
    (true, _) => "tasks.state.dailyBudget",
    (false, true) => "tasks.state.tokenLimit",
    (false, false) => "tasks.state.limit",
  }
}

const fn kind_glyph(kind: NeedKind) -> Icon {
  match kind {
    NeedKind::Approval | NeedKind::Input => Icon::Hand,
    NeedKind::Question => Icon::MessageCircle,
    NeedKind::PlanReview => Icon::Map,
    NeedKind::Decision | NeedKind::Interrupted | NeedKind::DeliveryBlocked => Icon::CirclePause,
    NeedKind::StoppedAtLimit => Icon::Gauge,
    NeedKind::ReadyToMerge => Icon::GitMerge,
    NeedKind::ReadyToComplete => Icon::CircleCheck,
    NeedKind::CommittedNothing => Icon::TriangleAlert,
    NeedKind::Conflicts => Icon::GitCompareArrows,
    NeedKind::DeliveryStopped => Icon::GitBranch,
    NeedKind::Memory => Icon::Brain,
  }
}

/// The words of what waits where its kind alone says it; a request and a plan are said with what they are about
/// ([`waits`]).
const fn kind_key(kind: NeedKind) -> &'static str {
  match kind {
    NeedKind::Question => "tasks.state.answer",
    NeedKind::Decision => "tasks.state.afterRetries",
    NeedKind::Approval | NeedKind::PlanReview | NeedKind::Input | NeedKind::Memory | NeedKind::DeliveryBlocked => {
      "tasks.state.input"
    }
    NeedKind::StoppedAtLimit => "tasks.state.limit",
    NeedKind::Interrupted => "tasks.state.interrupted",
    NeedKind::ReadyToMerge => "tasks.state.readyToMerge",
    NeedKind::ReadyToComplete => "tasks.state.readyToComplete",
    NeedKind::CommittedNothing => "tasks.state.committedNothing",
    NeedKind::Conflicts => "tasks.state.conflicts",
    NeedKind::DeliveryStopped => "tasks.state.deliveryStopped",
  }
}

/// Where a task nothing waits for stands.
fn stands(ctx: &Ctx, row: &TaskRow) -> Phrase {
  let said = message_text(ctx, &row.detail);
  let (glyph, glyph_color, color) = mark(row);
  let words = match (&row.readiness, row.run_step()) {
    (ReadinessKind::Run(TaskPhase::Running), Some(step)) => ctx.t_args("tasks.runStep", step_args(step)),
    (ReadinessKind::Run(TaskPhase::Failed), Some(step)) => ctx.t_args("tasks.state.failedAt", step_args(step)),
    (ReadinessKind::Unreadable, _) => ctx.t("tasks.state.unreadable"),
    _ => said,
  };
  Phrase {
    glyph,
    glyph_color,
    words,
    color,
  }
}

fn step_args(step: &CurrentStep) -> [(&'static str, String); 2] {
  [
    ("number", format!("{:02}", step.number)),
    ("step", step.name.to_string()),
  ]
}

/// The icon, its color and the words' color of what `row`'s words are about (`Status` in `x0VvUx`'s rows): what can
/// go ahead in `text_secondary` (ready to start `circle`, at work `loader-circle`, blocked `ban`), what only waits in
/// `text_muted` (queued `clock-3`, missing details `pencil-line`), a finished run in its outcome's tone, a run that
/// cannot be read in `danger`.
fn mark(row: &TaskRow) -> (Icon, PaletteColor, PaletteColor) {
  let secondary = |glyph| (glyph, PaletteColor::TextSecondary, PaletteColor::TextSecondary);
  let muted = |glyph| (glyph, PaletteColor::TextMuted, PaletteColor::TextMuted);
  let toned = |glyph, color: PaletteColor| (glyph, color.clone(), color);
  match &row.readiness {
    ReadinessKind::Ready | ReadinessKind::Run(TaskPhase::Draft) => {
      (Icon::Circle, PaletteColor::TextSecondary, PaletteColor::TextMuted)
    }
    ReadinessKind::Missing => muted(Icon::PencilLine),
    ReadinessKind::Held(hold) => held(hold),
    ReadinessKind::Blocked => (Icon::Ban, PaletteColor::TextSecondary, PaletteColor::TextMuted),
    ReadinessKind::NotShared => muted(Icon::Lock),
    ReadinessKind::Unreadable => toned(Icon::CircleHelp, PaletteColor::Danger),
    ReadinessKind::Run(phase) => match phase {
      TaskPhase::Draft | TaskPhase::Queued => muted(Icon::Clock3),
      TaskPhase::Running => secondary(Icon::LoaderCircle),
      TaskPhase::Waiting | TaskPhase::Stuck => toned(Icon::Hand, PaletteColor::Warning),
      TaskPhase::Paused | TaskPhase::Interrupted => toned(Icon::CirclePause, PaletteColor::Warning),
      TaskPhase::StoppedAtLimit => toned(Icon::Gauge, PaletteColor::Warning),
      TaskPhase::Succeeded => toned(Icon::CircleCheck, PaletteColor::Success),
      TaskPhase::Merged => secondary(Icon::GitMerge),
      TaskPhase::Failed => toned(Icon::CircleX, PaletteColor::Danger),
      TaskPhase::Cancelled => muted(Icon::CircleSlash),
    },
  }
}

/// A task its reviewed group holds: one that waits its turn, one held after something went wrong, or one its group
/// will not start.
fn held(hold: &GroupHold) -> (Icon, PaletteColor, PaletteColor) {
  match hold {
    GroupHold::AgentCapacity
    | GroupHold::DailyBudget
    | GroupHold::PreviousTask
    | GroupHold::Concurrency
    | GroupHold::Blocked { .. } => (Icon::Clock3, PaletteColor::TextMuted, PaletteColor::TextMuted),
    GroupHold::Canceled => (Icon::CircleSlash, PaletteColor::TextMuted, PaletteColor::TextMuted),
    GroupHold::SessionEnded
    | GroupHold::QueueHeld
    | GroupHold::Paused
    | GroupHold::GroupCap
    | GroupHold::FailedTask
    | GroupHold::Preflight(_)
    | GroupHold::StartFailed => (Icon::CirclePause, PaletteColor::Warning, PaletteColor::Warning),
  }
}
