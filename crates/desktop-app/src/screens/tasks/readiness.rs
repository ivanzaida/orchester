//! The icon and tone of a row's dependency / readiness cell (`Readiness cell` `Rcl9w` in `XEQuo`). Its words
//! are the workspace's (`orchester_workspace::tasks::browse::rows` picks them for each row); what they are
//! about decides how they are drawn, as the design's rows draw them: what can go ahead in `text_secondary`
//! (Ready to queue `circle-play`, Agent at work `bot`, Blocked `ban`), what waits on the person in
//! `warning` (Waiting for you `hand`, Raise the limit `gauge`, Held `circle-pause`), what only waits in
//! `text_muted` (Waiting for an agent slot `hourglass`, Needs a brief `pencil-line`), a finished run in its
//! outcome's tone (Every step passed `success`, Run failed `danger`), and a run that cannot be read in
//! `danger` with `file-warning`.

use std::collections::BTreeMap;

use lurq::app::theme::PaletteColor;
use orchester_core::{id::TaskId, run::TaskPhase, task_group::GroupHold};
use orchester_workspace::tasks::{
  group::hold_row_message,
  list::{Readiness, TaskRow},
};

use crate::ui::icon::Icon;

/// The icon before what `row`'s cell says, and the color of both. `holds` are how the project's reviewed
/// groups hold their tasks; `blocked` says another task blocks `row`'s or it is marked Blocked.
pub(super) fn mark(row: &TaskRow, holds: &BTreeMap<TaskId, GroupHold>, blocked: bool) -> (Icon, PaletteColor) {
  // A hold says so only where the workspace chose its words for the row.
  let canceled = GroupHold::Canceled;
  let hold = holds
    .get(&row.id)
    .into_iter()
    .chain([&canceled])
    .find(|hold| hold_row_message(hold) == row.detail);
  if let Some(hold) = hold {
    return held(hold);
  }
  if row.not_shared {
    return (Icon::Archive, PaletteColor::TextMuted);
  }
  if blocked {
    return (Icon::Ban, PaletteColor::TextSecondary);
  }
  match row.phase {
    Some(TaskPhase::Draft) if row.detail == Readiness::Ready.message() => {
      (Icon::CirclePlay, PaletteColor::TextSecondary)
    }
    Some(phase) => phase_mark(phase),
    None => (Icon::FileWarning, PaletteColor::Danger),
  }
}

/// A task its reviewed group holds: one that waits its turn, one held after something went wrong, or one
/// its group will not start.
const fn held(hold: &GroupHold) -> (Icon, PaletteColor) {
  match hold {
    GroupHold::AgentCapacity
    | GroupHold::DailyBudget
    | GroupHold::PreviousTask
    | GroupHold::Concurrency
    | GroupHold::Blocked { .. } => (Icon::Hourglass, PaletteColor::TextMuted),
    GroupHold::Canceled => (Icon::CircleSlash, PaletteColor::TextMuted),
    GroupHold::SessionEnded
    | GroupHold::GroupCap
    | GroupHold::FailedTask
    | GroupHold::Preflight(_)
    | GroupHold::StartFailed => (Icon::CirclePause, PaletteColor::Warning),
  }
}

/// Where a task in `phase` stands; a draft that is not ready still misses details.
const fn phase_mark(phase: TaskPhase) -> (Icon, PaletteColor) {
  match phase {
    TaskPhase::Draft => (Icon::PencilLine, PaletteColor::TextMuted),
    TaskPhase::Queued => (Icon::Hourglass, PaletteColor::TextMuted),
    TaskPhase::Running => (Icon::Bot, PaletteColor::TextSecondary),
    TaskPhase::Waiting | TaskPhase::Stuck => (Icon::Hand, PaletteColor::Warning),
    TaskPhase::Paused | TaskPhase::Interrupted => (Icon::CirclePause, PaletteColor::Warning),
    TaskPhase::StoppedAtLimit => (Icon::Gauge, PaletteColor::Warning),
    TaskPhase::Succeeded => (Icon::CircleCheck, PaletteColor::Success),
    TaskPhase::Merged => (Icon::GitMerge, PaletteColor::TextSecondary),
    TaskPhase::Failed => (Icon::CircleX, PaletteColor::Danger),
    TaskPhase::Cancelled => (Icon::CircleSlash, PaletteColor::TextMuted),
  }
}
