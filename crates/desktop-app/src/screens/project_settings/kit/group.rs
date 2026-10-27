//! A group (`Project` `nvlrw`, `Auto mode` `WSQqT`): its heading in 12/600 `text_secondary` with a count or an action
//! at its right (`Head` `o3JUr`, `CDSyw`), 8 over its rows in a box with a 1 `border` stroke and radius 10, each row
//! after the first under a 1 `border` line (`Rows` `V1Fos`). What a group says under its box (a question asked in
//! place, a note) follows the box, 8 under it.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::theme::ProjectSettingsText;

pub(crate) struct Group {
  id: Arc<str>,
  title: Arc<str>,
  end: Vec<Element>,
  rows: Vec<Element>,
  after: Vec<Element>,
}

impl Group {
  /// The group with the element id `id` under the heading `title`; the heading's id is `<id>-title`.
  pub(crate) fn new(id: &str, title: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      title: title.into(),
      end: Vec::new(),
      rows: Vec::new(),
      after: Vec::new(),
    }
  }

  /// What sits at the heading's right: a count, a note or an action.
  pub(crate) fn end(mut self, end: impl Into<Element>) -> Self {
    self.end.push(end.into());
    self
  }

  pub(crate) fn row(mut self, row: impl Into<Element>) -> Self {
    self.rows.push(row.into());
    self
  }

  pub(crate) fn rows(mut self, rows: impl IntoIterator<Item = Element>) -> Self {
    self.rows.extend(rows);
    self
  }

  /// What the group says under its box.
  pub(crate) fn after(mut self, after: impl Into<Element>) -> Self {
    self.after.push(after.into());
    self
  }
}

impl From<Group> for Element {
  fn from(group: Group) -> Self {
    let heading = Row::new()
      .id(format!("{}-head", group.id))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&group.title)
          .id(format!("{}-title", group.id))
          .variant(ProjectSettingsText::GroupTitle.role())
          .flex(1.0),
      )
      .with_children(group.end);
    let boxed = (!group.rows.is_empty()).then(|| {
      Column::new()
        .id(format!("{}-rows", group.id))
        .width(Dimension::Pct(100.0))
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Lg)
        .clip()
        .with_children(group.rows.into_iter().enumerate().map(|(index, row)| {
          let line = Column::new().width(Dimension::Pct(100.0)).child(row);
          if index == 0 {
            line
          } else {
            line.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
          }
        }))
    });
    Column::new()
      .id(group.id.to_string())
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(heading)
      .with_children(boxed)
      .with_children(group.after)
      .into()
  }
}
