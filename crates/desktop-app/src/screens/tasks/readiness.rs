//! The icon and tone of a row's dependency / readiness cell (`Readiness cell` `Rcl9w` in `XEQuo`). Its words
//! are the workspace's (`orchester_workspace::tasks::browse::rows` picks them for each row, with what they
//! are about, `ReadinessKind`); what they are about decides how they are drawn, as the design's rows draw
//! them: what can go ahead in `text_secondary` (Ready to queue `circle-play`, Agent at work `bot`, Blocked `ban`), what
//! waits on the person in `warning` (Waiting for you `hand`, Raise the limit `gauge`, Held `circle-pause`), what only
//! waits in `text_muted` (Waiting for an agent slot `hourglass`, Needs a brief `pencil-line`), a finished run in its
//! outcome's tone (Every step passed `success`, Run failed `danger`), and a run that cannot be read in
//! `danger` with `file-warning`.

use lurq::app::theme::PaletteColor;
use orchester_core::{run::TaskPhase, task_group::GroupHold};
use orchester_workspace::tasks::list::{ReadinessKind, TaskRow};

use crate::ui::icon::Icon;

/// The icon before what `row`'s cell says, and the color of both, by what the cell is about.
pub(super) fn mark(row: &TaskRow) -> (Icon, PaletteColor) {
  match &row.readiness {
    ReadinessKind::Ready => (Icon::CirclePlay, PaletteColor::TextSecondary),
    ReadinessKind::Missing => (Icon::PencilLine, PaletteColor::TextMuted),
    ReadinessKind::Held(hold) => held(hold),
    ReadinessKind::Blocked => (Icon::Ban, PaletteColor::TextSecondary),
    ReadinessKind::NotShared => (Icon::Archive, PaletteColor::TextMuted),
    ReadinessKind::Run(phase) => phase_mark(*phase),
    ReadinessKind::Unreadable => (Icon::FileWarning, PaletteColor::Danger),
  }
}

/// Whether the row carries the Blocked flag: another task blocks it, or it is marked Blocked (also while
/// its group's hold is what its cell says).
pub(super) fn blocked(row: &TaskRow) -> bool {
  row.readiness == ReadinessKind::Blocked || row.organization.blocked_reason().is_some()
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

/// Where a task in `phase` stands.
const fn phase_mark(phase: TaskPhase) -> (Icon, PaletteColor) {
  match phase {
    TaskPhase::Draft => (Icon::CirclePlay, PaletteColor::TextSecondary),
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
