//! *Runs* (`Runs` `Pr5cn` in `01b · Summary · full page`): every run of the task, newest first, 6 apart, under its
//! heading with their count: the run the page shows on `surface_raised`, each with where it stands, when, the tokens it
//! used, what its worktree holds and its `…` ([`row`]). A run that cannot be read is listed with its number alone. A
//! run's removal confirmation opens under its row (`removal`).

pub mod line;
pub mod removal;
mod row;

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::RunId, run::TaskPhase};
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
  // The step of the latest run whose agent waits for the person's approval, by name.
  let approval = runs.task.waiting_approval().and_then(|approval| {
    let run = detail.run.as_ref()?;
    Some(run.step_of(approval.approval.execution)?.name.clone())
  });
  let rows: Vec<Element> = detail
    .history
    .iter()
    .map(|record| {
      let props = row_props(runs, record, (latest, shown), approval.clone());
      item(ctx, runs, props)
    })
    .collect();
  Some(
    Column::new()
      .id("task-runs")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
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
          .spacing(ExtraSpacing::Snug.role())
          .with_children(rows),
      )
      .into(),
  )
}

/// What `record`'s row shows: the run the page shows is `shown`, the task's latest run `latest`; `approval` is the
/// step of the latest run waiting for an approval.
fn row_props(
  runs: &Runs,
  record: &RunRecord,
  (latest, shown): (Option<RunId>, Option<RunId>),
  approval: Option<Arc<str>>,
) -> RowProps {
  let lost = runs.task.lost();
  let live = record.run.as_ref().is_some_and(|run| run.status().has_live_work());
  let mut record = record.clone();
  // Without a runner the run's agent is gone: it is interrupted, as the header and the rail say.
  if lost && live {
    record.phase = record.phase.map(TaskPhase::without_runner);
  }
  // A request its agent waits on: the run needs the person, as the header and the rail say.
  let waits = approval.filter(|_| Some(record.id) == latest);
  if waits.is_some() {
    record.phase = Some(TaskPhase::Waiting);
  }
  let task = runs.task;
  let history = &task.detail.history;
  RowProps {
    project: task.project.clone(),
    key: task.detail.task.key().clone(),
    kept: task.detail.not_shared(),
    left: task
      .left
      .as_read(record.id)
      .filter(|_| record.without_live_work().is_some()),
    removable: record
      .without_live_work()
      .and(task.left.of(record.id))
      .and_then(Result::ok)
      .filter(|left| !left.worktrees.is_empty()),
    this_run: Some(record.id) == shown,
    job: task.jobs.run(record.id),
    others_busy: history
      .iter()
      .any(|other| other.id != record.id && task.jobs.run(other.id).busy()),
    confirming: runs.removal.run == Some(record.id),
    lost: lost && live,
    waits_for_approval: waits,
    record,
  }
}

/// A run's row and, under it while the row's menu opened it, its removal's confirmation.
fn item(ctx: &mut Ctx, runs: &Runs, props: RowProps) -> Element {
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
        .padding(Padding::new().top(SpacingSize::Sm).bottom(SpacingSize::Md))
        .child(confirmation)
    });
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(row)
    .with_children(confirmation)
    .into()
}
