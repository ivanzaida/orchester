//! A group (`Project` `nvlrw`, `Auto mode` `WSQqT`): its heading in 12/600 `text_secondary` with a count or an action
//! at its right (`Head` `o3JUr`, `CDSyw`) or a count right after it (`Head` `I5nZS`), 8 over its rows in a box with a 1
//! `border` stroke and radius 10, each row after the first under a 1 `border` line (`Rows` `V1Fos`). A toolbar may sit
//! between the heading and the box, 8 from each (`Toolbar` `jSvl2` in `S06`). What a group says under its box (its
//! foot, then a question asked in place or a note) follows the box, 8 under it.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraSpacing, IconSize, ProjectSettingsText},
  ui::icon::{Icon, icon},
};

pub(crate) struct Group {
  id: Arc<str>,
  title: Arc<str>,
  beside: Vec<Element>,
  end: Vec<Element>,
  over: Vec<Element>,
  rows: Vec<Element>,
  foot: Option<Element>,
  after: Vec<Element>,
}

impl Group {
  /// The group with the element id `id` under the heading `title`; the heading's id is `<id>-title`.
  pub(crate) fn new(id: &str, title: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      title: title.into(),
      beside: Vec::new(),
      end: Vec::new(),
      over: Vec::new(),
      rows: Vec::new(),
      foot: None,
      after: Vec::new(),
    }
  }

  /// What sits between the heading and the box: a toolbar over the rows.
  pub(crate) fn over(mut self, over: impl Into<Element>) -> Self {
    self.over.push(over.into());
    self
  }

  /// What sits right after the heading, 8 from it: a count (`4 secrets` in `06`).
  pub(crate) fn beside(mut self, beside: impl Into<Element>) -> Self {
    self.beside.push(beside.into());
    self
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

  /// The group's foot (`Foot` `VTUGP` in `S02`): an `info` glyph and `words` right under the box, before whatever
  /// else the group says there; its id is `<id>-foot`, its words' `<id>-foot-text`.
  pub(crate) fn foot(mut self, words: &str) -> Self {
    self.foot = Some(foot(&format!("{}-foot", self.id), words).into());
    self
  }

  /// What the group says under its box.
  pub(crate) fn after(mut self, after: impl Into<Element>) -> Self {
    self.after.push(after.into());
    self
  }
}

/// A group's foot: a 12 `info` glyph and the words in 12 `text_muted`, 6 apart, 2 around them (`Foot` `VTUGP`).
fn foot(id: &str, words: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(ExtraSpacing::Tight.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(ProjectSettingsText::Hint.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

impl From<Group> for Element {
  fn from(group: Group) -> Self {
    let title = Text::new(&group.title)
      .id(format!("{}-title", group.id))
      .variant(ProjectSettingsText::GroupTitle.role());
    // With something beside it, the title keeps its width and the room after it goes to a spacer.
    let (title, spacer) = if group.beside.is_empty() {
      (title.flex(1.0), None)
    } else {
      (title, Some(Spacer::new().flex(1.0)))
    };
    let heading = Row::new()
      .id(format!("{}-head", group.id))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(title)
      .with_children(group.beside)
      .with_children(spacer)
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
      .with_children(group.over)
      .with_children(boxed)
      .with_children(group.foot)
      .with_children(group.after)
      .into()
  }
}
