//! *Runs of this task* (`Runs of this task` `HYNSL` in `lUXff`): every run of the task, newest first,
//! the one the page shows tagged *this run*, each with where it stands, what happened, its worktree and
//! when, and *Remove worktree* while it has worktrees and no live work ([`row`]). A run that cannot be
//! read is listed with its number alone.

pub mod line;
pub mod removal;
mod row;

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::Column,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::RunId,
  run::{RunNumber, TaskPhase},
};
use orchester_workspace::tasks::history::RunRecord;

use self::row::{RowProps, RunRow};
use super::section_head;
use crate::{
  screens::task::{Removal, RemovalPlace, TaskContext},
  theme::ExtraSpacing,
};

/// What the section is drawn from.
pub struct Runs<'a> {
  pub task: &'a TaskContext,
  pub removal: Removal,
}

/// The section; `None` for a task without runs.
pub fn section(ctx: &mut Ctx, runs: &Runs) -> Option<Element> {
  let detail = &runs.task.detail;
  if detail.history.is_empty() {
    return None;
  }
  let latest = detail.run.as_ref().map(|run| run.id());
  let shown = latest.or_else(|| detail.latest_unreadable.as_ref().map(|run| run.id));
  let working = detail
    .history
    .iter()
    .filter_map(|record| record.run.as_ref())
    .find(|run| run.status().has_live_work())
    .map(|run| run.number());
  // The step of the latest run whose agent waits for the person's approval, by name.
  let approval = runs.task.waiting_approval().and_then(|approval| {
    let run = detail.run.as_ref()?;
    Some(run.step_of(approval.approval.execution)?.name.clone())
  });
  let rows: Vec<Element> = detail
    .history
    .iter()
    .enumerate()
    .map(|(index, record)| {
      let props = row_props(runs, record, (latest, shown), (working, approval.clone()));
      item(ctx, runs, props, index > 0)
    })
    .collect();
  Some(
    Column::new()
      .id("task-runs")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(section_head(
        ctx,
        "task-runs-head",
        "summary.runs.title",
        Some(detail.history.len()),
      ))
      .child(
        Column::new()
          .id("task-run-rows")
          .width(Dimension::Pct(100.0))
          .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
          .rounded(RadiusSize::Lg)
          .clip()
          .with_children(rows),
      )
      .into(),
  )
}

/// What `record`'s row shows: the run the page shows is `shown`, the task's latest run `latest`;
/// `working` is the task's run at work, `approval` the step of the latest run waiting for an approval.
fn row_props(
  runs: &Runs,
  record: &RunRecord,
  (latest, shown): (Option<RunId>, Option<RunId>),
  (working, approval): (Option<RunNumber>, Option<Arc<str>>),
) -> RowProps {
  let lost = runs.task.lost();
  let live = record.run.as_ref().is_some_and(|run| run.status().has_live_work());
  let mut record = record.clone();
  // Without a runner the run's agent is gone: it is interrupted, as the header and steps say.
  if lost && live {
    record.phase = record.phase.map(TaskPhase::without_runner);
  }
  // A request its agent waits on: the run needs the person, as the header and the steps say.
  let waits = approval.filter(|_| Some(record.id) == latest);
  if waits.is_some() {
    record.phase = Some(TaskPhase::Waiting);
  }
  let history = &runs.task.detail.history;
  RowProps {
    project: runs.task.project.clone(),
    key: runs.task.detail.task.key().to_string().into(),
    left: runs
      .task
      .left
      .as_read(record.id)
      .filter(|_| record.without_live_work().is_some()),
    // The latest run's is in the header.
    removable: record
      .without_live_work()
      .filter(|_| Some(record.id) != latest)
      .and(runs.task.left.of(record.id))
      .and_then(Result::ok)
      .filter(|left| !left.worktrees.is_empty()),
    this_run: Some(record.id) == shown,
    job: runs.task.jobs.run(record.id),
    others_busy: history
      .iter()
      .any(|other| other.id != record.id && runs.task.jobs.run(other.id).busy()),
    working: working.filter(|number| Some(*number) != record.number),
    confirming: runs.removal.run == Some(record.id),
    lost: lost && live,
    waits_for_approval: waits,
    record,
  }
}

/// A run's row and, under it while the row's button opened it, its removal's confirmation; on a line
/// after the first.
fn item(ctx: &mut Ctx, runs: &Runs, props: RowProps, divided: bool) -> Element {
  let run = props.record.id;
  let row = ctx.mount_keyed::<RunRow>(&format!("task-run-row-{run}"), props);
  let confirmation = runs
    .removal
    .shows(run, RemovalPlace::Row)
    .then(|| removal::confirm_for(ctx, runs, run))
    .flatten()
    .map(|confirmation| {
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .left(ExtraSpacing::Inset.role())
            .right(ExtraSpacing::Inset.role())
            .bottom(SpacingSize::Md),
        )
        .child(confirmation)
    });
  let item = Column::new()
    .width(Dimension::Pct(100.0))
    .child(row)
    .with_children(confirmation);
  if divided {
    item
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .into()
  } else {
    item.into()
  }
}
