//! *Runs of this task* (`Runs of this task` `HYNSL` in `lUXff`): every run of the task, newest first,
//! the one the page shows tagged *this run*, each with where it stands, what happened, its worktree and
//! when, and *Remove worktree* while it has worktrees and no live work ([`row`]). A run that cannot be
//! read is listed with its number alone.

mod line;
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
use orchester_workspace::tasks::{detail::TaskDetail, integration_jobs::ProjectJobState};

use self::row::{RowProps, RunRow};
use super::section_head;
use crate::{
  project::ShownProject,
  screens::task::{Removal, RemovalPlace, read::TaskLeftovers},
  theme::ExtraSpacing,
};

/// What the section is drawn from.
pub struct Runs<'a> {
  pub project: &'a ShownProject,
  pub detail: &'a TaskDetail,
  pub left: &'a TaskLeftovers,
  pub jobs: &'a ProjectJobState,
  pub removal: Removal,
}

/// The section; `None` for a task without runs.
pub fn section(ctx: &mut Ctx, runs: &Runs) -> Option<Element> {
  let detail = runs.detail;
  if detail.history.is_empty() {
    return None;
  }
  let shown = detail
    .run
    .as_ref()
    .map(|run| run.id())
    .or_else(|| detail.latest_unreadable.as_ref().map(|run| run.id));
  let working = detail
    .history
    .iter()
    .filter_map(|record| record.run.as_ref())
    .find(|run| run.status().has_live_work())
    .map(|run| run.number());
  let key: Arc<str> = detail.task.key().to_string().into();
  let rows: Vec<Element> = detail
    .history
    .iter()
    .enumerate()
    .map(|(index, record)| {
      let props = RowProps {
        project: runs.project.clone(),
        key: key.clone(),
        record: record.clone(),
        left: runs.left.as_read(record.id).filter(|_| record.without_live_work().is_some()),
        removable: record
          .without_live_work()
          .and(runs.left.of(record.id))
          .and_then(Result::ok)
          .filter(|left| !left.worktrees.is_empty()),
        this_run: Some(record.id) == shown,
        job: runs.jobs.run(record.id),
        others_busy: detail
          .history
          .iter()
          .any(|other| other.id != record.id && runs.jobs.run(other.id).busy()),
        working: working.filter(|number| Some(*number) != record.number),
        confirming: runs.removal.run == Some(record.id),
      };
      item(ctx, runs, props, index > 0)
    })
    .collect();
  Some(
    Column::new()
      .id("task-runs")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(section_head(ctx, "task-runs-head", "summary.runs.title", Some(detail.history.len())))
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
    item.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border)).into()
  } else {
    item.into()
  }
}
