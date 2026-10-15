//! A reply's labelled links as their grid (`Links` `ZZbyh` in `w262B`): a row per link, 8 apart with 2 over and under
//! the grid, each its label in 12 `text_muted` in an 84 column, then the link, then its detail in 12.5
//! `text_secondary`, 12 apart. The rule for which list is one is `ui::chat::labelled`'s.

use lurq::{
  app::theme::SpacingSize,
  components::{Column, MarkdownList, Row, Text},
  layout::{Alignment, text_style::TextStyle},
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraSpacing, OrchestratorSpacing, OrchestratorText},
  ui::chat::labelled::{LabelledLink, labelled_links},
};

/// The label's column (`p0bb6` in `w262B`).
const LABEL_WIDTH: f32 = 84.0;

/// `list` as a grid of labelled links, or `None` when it is not one: the document draws it as a list then.
/// `detail` is the details' text style.
pub(super) fn grid(list: &MarkdownList<'_>, detail: &TextStyle) -> Option<Element> {
  let rows = labelled_links(list.items())?;
  let grid = Column::new()
    .class("orchestrator-link-grid")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Tight.role())
        .bottom(ExtraSpacing::Tight.role()),
    )
    .spacing(SpacingSize::Sm)
    .with_children(rows.iter().map(|row| grid_row(list, row, detail)));
  Some(grid.into())
}

fn grid_row(list: &MarkdownList<'_>, row: &LabelledLink<'_>, detail: &TextStyle) -> Element {
  // Content-sized, so the detail follows the link's words rather than the end of the row.
  let link = Row::new()
    .flex_shrink(1.0)
    .child(list.inlines(std::slice::from_ref(row.link), list.style()));
  let mut line = Row::new()
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
  if !row.detail.is_empty() {
    line = line.child(
      Row::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .child(list.inlines(&row.detail, detail)),
    );
  }
  line.into()
}
