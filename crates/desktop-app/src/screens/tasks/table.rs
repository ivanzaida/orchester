//! The task table (`Table` `pJQfB`): its header (`Task list header` `FjOYw`), its rows, as many as the panel
//! has room for and the rest scrolled, and the count under it (`Footer` `uTRHT`). The table is as tall as
//! its rows, so the count follows the last one. Rows are windowed: a project's list grows past what a
//! screen holds.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text, VirtualizedList},
  layout::layout_kind::ScrollState,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::{data::ProjectTasks, list, list::TaskRow};

use super::row::{RowProps, TaskListRow};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    message::message_text,
    table::{ColumnWidth, HEADER_HEIGHT, ROW_HEIGHT, table_header},
  },
};

/// The columns' widths (`FjOYw`): ID 52, the task the room left, Pipeline 128, Priority 80, Workflow / run
/// 168, Dependency / readiness 188.
pub(super) const KEY: ColumnWidth = ColumnWidth::Fixed(52.0);
pub(super) const TASK: ColumnWidth = ColumnWidth::Fill;
pub(super) const PIPELINE: ColumnWidth = ColumnWidth::Fixed(128.0);
pub(super) const PRIORITY: ColumnWidth = ColumnWidth::Fixed(80.0);
pub(super) const WORKFLOW: ColumnWidth = ColumnWidth::Fixed(168.0);
pub(super) const READINESS: ColumnWidth = ColumnWidth::Fixed(188.0);

/// The table's header, with the columns' headings.
pub(super) fn header(ctx: &Ctx) -> Row {
  let headings = [
    (KEY, "tasks.columns.key"),
    (TASK, "tasks.columns.task"),
    (PIPELINE, "tasks.columns.pipeline"),
    (PRIORITY, "tasks.columns.priority"),
    (WORKFLOW, "tasks.columns.workflow"),
    (READINESS, "tasks.columns.readiness"),
  ]
  .map(|(width, key)| (width, ctx.t(key)));
  let columns: Vec<(ColumnWidth, Option<&str>)> = headings
    .iter()
    .map(|(width, heading)| (*width, Some(&**heading)))
    .collect();
  table_header("tasks-table-header", &columns, true)
}

/// What the table shows: the rows, the task Create just added, and the project they are of.
pub(super) struct Rows<'a> {
  pub project: ProjectId,
  pub tasks: &'a ProjectTasks,
  pub rows: Arc<Vec<TaskRow>>,
  pub highlight: Option<&'a str>,
  /// Where the list is scrolled to (`super::TasksScreen::place`).
  pub scroll: ScrollState,
  /// The list's key, new for another tab, search, pipeline or sort: a list keeps its top row in place
  /// while its rows change, which a new list must not.
  pub list_key: String,
}

/// The header over the rows, scrolled where `shown.scroll` says.
pub(super) fn table(ctx: &mut Ctx, shown: &Rows<'_>) -> Element {
  let count = shown.rows.len();
  let highlight: Option<Arc<str>> = shown
    .highlight
    .filter(|key| shown.rows.iter().any(|row| &*row.key == *key))
    .map(Arc::from);
  let project = shown.project;
  let rows = VirtualizedList::new_shared(ctx, shown.rows.clone())
    .list_key(shown.list_key.clone())
    .with_scroll_state(shown.scroll.clone())
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .max_height(Dimension::Px(ROW_HEIGHT * count as f32))
    .mount_keyed::<TaskListRow, _, _, _>(
      |row| row.key.clone(),
      move |row| RowProps {
        project,
        row: row.clone(),
        highlighted: highlight.as_deref() == Some(&*row.key),
      },
    );
  Column::new()
    .id("tasks-table")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .max_height(Dimension::Px(HEADER_HEIGHT + ROW_HEIGHT * count as f32))
    .child(header(ctx))
    .child(rows)
    .into()
}

/// What the count under the rows says besides how many there are.
pub(super) enum Count {
  /// The list's tasks, with the runs they started or how many match.
  Tasks,
  /// Found by their keys before joining (`1 task · found by its key before joining`).
  FormerKey,
  /// The *Not shared* tasks, of these many (`1 of 2 Not shared tasks`).
  NotShared(usize),
}

/// How many rows there are and how many runs they started, or how many match while the search or the
/// pipeline narrows them; the *Not shared* ones, or those found by their keys before joining, as such.
pub(super) fn footer(ctx: &Ctx, shown: &Rows<'_>, (filtered, kind): (bool, Count)) -> Row {
  let rows = shown.rows.len();
  let text = match kind {
    Count::Tasks => message_text(ctx, &list::footer(rows, shown.tasks.runs_of(&shown.rows), filtered)),
    Count::FormerKey => counted(
      ctx,
      (
        "tasks.notShared.foundByFormer.one",
        "tasks.notShared.foundByFormer.other",
      ),
      rows,
    ),
    Count::NotShared(total) if rows == total => counted(
      ctx,
      ("tasks.notShared.footer.one", "tasks.notShared.footer.other"),
      rows,
    ),
    Count::NotShared(total) => ctx.t_args(
      "tasks.notShared.footerOf",
      [("count", rows.to_string()), ("total", total.to_string())],
    ),
  };
  Row::new()
    .id("tasks-footer")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .left(SpacingSize::Md)
        .right(SpacingSize::Md)
        .bottom(ExtraSpacing::Loose.role()),
    )
    .child(
      Text::new(&text)
        .id("tasks-footer-text")
        .variant(ExtraTypography::Note.role()),
    )
}

/// `one` or `other` with `count`.
fn counted(ctx: &Ctx, (one, other): (&str, &str), count: usize) -> Arc<str> {
  let key = if count == 1 { one } else { other };
  ctx.t_args(key, [("count", count.to_string())])
}
