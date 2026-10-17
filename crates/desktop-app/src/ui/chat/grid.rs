//! An agent's labelled links as their grid (`Links` `ZZbyh` in `w262B`): a row per link, 8 apart with 2 over and
//! under the grid, each its label in 12 `text_muted` in an 84 column, then the link, then its detail in 12.5
//! `text_secondary`, 12 apart. The rule for which list is one is [`super::labelled`]'s.

use lurq::{
  app::theme::SpacingSize,
  components::{Column, MarkdownList, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::labelled::{LabelledLink, labelled_links};
use crate::theme::{ExtraSpacing, OrchestratorSpacing, OrchestratorText};

/// The label's column (`p0bb6` in `w262B`).
const LABEL_WIDTH: f32 = 84.0;
/// A row too long for its line: the detail wraps first, down to this width, and only then the link.
const DETAIL_WIDTH: f32 = 200.0;
const LINK_ORDER: i32 = 1;

/// `list` as a grid of labelled links, or `None` when it is not one: the document draws it as a list then.
pub(super) fn grid(list: &MarkdownList<'_>) -> Option<Element> {
  let rows = labelled_links(list.items())?;
  let grid = Column::new()
    .overflow_visible()
    .class("chat-link-grid")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Tight.role())
        .bottom(ExtraSpacing::Tight.role()),
    )
    .spacing(SpacingSize::Sm)
    .with_children(rows.iter().map(|row| grid_row(list, row)));
  Some(grid.into())
}

/// The label, the link as the document draws it (presented, focusable, followed), and the detail, plain text as the
/// design draws it. Both are as wide as their words, the detail taking the rest of the row after the link; when they
/// do not fit the row, the detail wraps first, then the link.
fn grid_row(list: &MarkdownList<'_>, row: &LabelledLink<'_>) -> Element {
  // The focused link's ring reaches past the cell and the row: neither clips it, as the paragraph's own containers
  // do not (`Mjhop` in `agv8r`).
  let link = Row::new()
    .overflow_visible()
    .flex_shrink(1.0)
    .shrink_order(LINK_ORDER)
    .min_width(Dimension::Px(0.0))
    .child(list.inlines(std::slice::from_ref(row.link), list.style()));
  let mut line = Row::new()
    .overflow_visible()
    .width(Dimension::Pct(100.0))
    .spacing(OrchestratorSpacing::LinkColumns.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&row.label)
        .variant(OrchestratorText::LinkLabel.role())
        .width(Dimension::Px(LABEL_WIDTH))
        .flex_shrink(0.0),
    )
    .child(link);
  let detail = row.detail_text();
  if !detail.is_empty() {
    line = line.child(
      Text::new(&detail)
        .variant(OrchestratorText::LinkDetail.role())
        .selectable(true)
        .flex_full(1.0, 1.0, None)
        .min_width(Dimension::Px(DETAIL_WIDTH)),
    );
  }
  line.into()
}
