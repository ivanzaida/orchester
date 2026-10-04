//! An interrupted run whose worktree was removed (ORC-63, 63.05 `LmTnc`): it stays *Interrupted* in
//! its record, but its agent has nothing to continue from, so the notice says it can't continue,
//! *Continue run* stays in the topbar unavailable with the lock line under the notice saying why,
//! and *Run again* is offered.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{run::RunStatus, time::Timestamp};

use crate::{
  display,
  tasks::detail::{RunView, TaskDetail},
  theme::ExtraSpacing,
  ui::{icon::Icon, notice::lock_line},
};

/// When the worktree of `run` was removed, while it is interrupted: it can never continue.
pub(super) fn removed_at(run: &RunView) -> Option<Timestamp> {
  matches!(run.run.status(), RunStatus::Interrupted(_))
    .then_some(run.worktree_removed)
    .flatten()
}

/// "Run #141 can't continue" and why (`LmTnc`), for an interrupted run whose worktree was removed.
pub(super) fn notice(ctx: &Ctx, run: &RunView) -> Option<(Icon, Arc<str>, Arc<str>)> {
  let removed = removed_at(run)?;
  let number = run.run.number().get().to_string();
  Some((
    Icon::FolderX,
    ctx.t_args("runs.summary.notice.cantContinueTitle", [("run", number.as_str())]),
    ctx.t_args(
      "runs.summary.notice.cantContinueDetail",
      [
        ("time", display::clock(removed).as_str()),
        ("branch", run.run.branch().as_str()),
      ],
    ),
  ))
}

/// Why *Continue run* is unavailable, under the notice: the run's worktree was removed.
pub(super) fn continue_unavailable(ctx: &Ctx, detail: &TaskDetail) -> Option<Element> {
  let run = detail.run.as_ref()?;
  removed_at(run)?;
  let number = run.run.number().get().to_string();
  Some(lock_line(
    "task-continue-unavailable",
    &ctx.t_args("runs.page.continueUnavailable", [("run", number.as_str())]),
  ))
}

/// The lines under the run's notice that say why an action is unavailable, one under the other.
pub(super) fn reasons(lines: impl IntoIterator<Item = Element>) -> Option<Element> {
  let mut lines: Vec<Element> = lines.into_iter().collect();
  match lines.len() {
    0 => None,
    1 => lines.pop(),
    _ => Some(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Stack.role())
        .with_children(lines)
        .into(),
    ),
  }
}
