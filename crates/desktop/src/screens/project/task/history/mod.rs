//! *Runs of this task* (ORC-63, 63.01 `O0AMKY`, at 960 63.02 `H9Z7D4`): a Summary section after the
//! run's steps that lists every run of the task, newest first, the shown one tagged *this run*,
//! each with where it stands, what happened, its worktree and when, and *Remove worktree* while it
//! has a worktree and no live work (see [`row`]). It replaces the unreadable run page's *Other runs
//! of this task* (02.R `SUuwV`).

mod line;
mod row;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use self::row::{RunRow, RunRowProps};
use super::worktree::{TaskLeftovers, others, working};
use crate::{
  tasks::detail::TaskDetail,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  workspace::OpenProject,
};

/// The section for `detail`, whose runs without live work left what `left` says; `None` for a task
/// without runs.
pub(super) fn section(
  ctx: &mut Ctx,
  open: &OpenProject,
  detail: &TaskDetail,
  left: Option<&TaskLeftovers>,
) -> Option<Element> {
  if detail.history.is_empty() {
    return None;
  }
  // Reading the window's width re-renders the page as the window resizes.
  let wide = ctx.window().logical_width() >= crate::theme::RUN_HISTORY_COLUMNS_MIN_WINDOW;
  let latest = detail.run.as_ref().map(|run| run.id());
  let shown = latest.or_else(|| detail.latest_unreadable.as_ref().map(|run| run.id));
  let working = working(detail);
  let key: std::sync::Arc<str> = detail.task.key().to_string().into();
  let rows: Vec<Element> = detail
    .history
    .iter()
    .map(|record| {
      let props = RunRowProps {
        open: open.clone(),
        task: detail.task.id(),
        key: key.clone(),
        record: record.clone(),
        left: left.and_then(|left| left.as_read(record.id)),
        this_run: Some(record.id) == shown,
        // The topbar's Remove worktree confirms the latest run's removal.
        dialog: Some(record.id) != latest,
        others: others(detail, record.id),
        working,
        wide,
      };
      ctx.mount_keyed::<RunRow>(&format!("task-run-row-{}", record.id), props)
    })
    .collect();
  let count = detail.history.len().to_string();
  Some(
    Column::new()
      .id("task-runs")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(Text::new(&ctx.t("runs.history.title")).variant(ExtraTypography::SectionHeading.role()))
          .child(Text::new(&count).variant(ExtraTypography::NavCount.role())),
      )
      .child(
        Text::new(&ctx.t("runs.history.explanation"))
          .variant(ExtraTypography::Detail.role())
          .width(Dimension::Pct(100.0)),
      )
      .child(
        Column::new()
          .id("task-run-rows")
          .width(Dimension::Pct(100.0))
          .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
          .with_children(rows),
      )
      .into(),
  )
}
