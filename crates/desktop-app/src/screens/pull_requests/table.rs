//! The pull request table (`Table` `H0geM`): its header (`Pull request list header` `ODk8t`), its rows, as
//! many as the panel has room for and the rest scrolled, and the count under it (`Footer` `y9Qm1J`). The table
//! is as tall as its rows, so the count follows the last one.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text, VirtualizedList},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::{messages::Message, pull_requests::list::ListedPullRequest};

use super::{
  row::{CHECKS, OPEN, PULL_REQUEST, PullRequestRow, ROW_HEIGHT, RowProps, STATE},
  work::OpenAction,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    message::message_text,
    table::{ColumnWidth, HEADER_HEIGHT, table_header},
  },
};

/// The table's header, with the columns' headings; the Open in GitHub column has none.
pub(super) fn header(ctx: &Ctx) -> Row {
  let headings = [
    ctx.t("pull_requests.columns.pullRequest"),
    ctx.t("pull_requests.columns.state"),
    ctx.t("pull_requests.columns.checks"),
  ];
  let columns: [(ColumnWidth, Option<&str>); 4] = [
    (PULL_REQUEST, Some(&headings[0])),
    (STATE, Some(&headings[1])),
    (CHECKS, Some(&headings[2])),
    (OPEN, None),
  ];
  table_header("pull-requests-table-header", &columns, true)
}

/// What the table shows.
pub(super) struct Rows {
  pub project: ProjectId,
  pub rows: Arc<Vec<ListedPullRequest>>,
  pub now: Timestamp,
  pub open: OpenAction,
  /// The list's key, new for another tab, search or sort: a list keeps its top row in place while its rows
  /// change, which a new list must not.
  pub list_key: String,
}

/// The header over the rows.
pub(super) fn table(ctx: &mut Ctx, shown: Rows) -> Element {
  let count = shown.rows.len();
  let Rows { project, now, open, .. } = shown;
  let rows = VirtualizedList::new_shared(ctx, shown.rows)
    .list_key(shown.list_key)
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .max_height(Dimension::Px(ROW_HEIGHT * count as f32))
    .mount_keyed::<PullRequestRow, _, _, _>(
      |listed| listed.pull_request.number().to_string(),
      move |listed| RowProps {
        project,
        listed: listed.clone(),
        now,
        open: open.clone(),
      },
    );
  Column::new()
    .id("pull-requests-table")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .max_height(Dimension::Px(HEADER_HEIGHT + ROW_HEIGHT * count as f32))
    .child(header(ctx))
    .child(rows)
    .into()
}

/// How many rows there are, of how many, and where they come from.
pub(super) fn footer(ctx: &Ctx, count: &Message) -> Row {
  Row::new()
    .id("pull-requests-footer")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .left(SpacingSize::Md)
        .right(SpacingSize::Md)
        .bottom(ExtraSpacing::Loose.role()),
    )
    .child(Text::new(&message_text(ctx, count)).variant(ExtraTypography::Note.role()))
}
