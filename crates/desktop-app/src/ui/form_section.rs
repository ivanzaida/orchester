//! The form's building blocks (`Task form` `FXCiX`, `Shared components · Forms` `DMMLd`): a section's
//! title over its rows (`Section` `De1nE`: 13/600, a `border` line 10 under it, a 11 `text_muted` note at
//! its end), a row with its label and description at the start and its control at the end (`Form row`
//! `XaNIz`: 12 over and under, 24 between, the label 13/500 with its description 3 under it), a row
//! whose control runs under its label (`WXgjE`, 10 apart), and a feature that is not built yet, its
//! button faded with a lock and why under it.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraSpacing, ExtraTypography, TaskFormText},
  ui::{
    button::{ActionButton, ButtonKind},
    unavailable::with_reason,
  },
};

/// A section's title and its rows; the title's element id is `<id>-title`.
pub fn section(id: &str, title: &str, note: Option<&str>, rows: Vec<Element>) -> Column {
  let heading = Row::new()
    .id(format!("{id}-heading"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .padding(Padding::new().bottom(ExtraSpacing::Loose.role()))
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(TaskFormText::SectionTitle.role()),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(note.map(|note| {
      Text::new(note)
        .id(format!("{id}-note"))
        .variant(TypographyStyle::Caption)
    }));
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .child(heading)
    .with_children(rows)
}

/// What a row says about its field: the label and, under it, what the field is for.
pub struct Label {
  pub id: Arc<str>,
  pub label: Arc<str>,
  pub description: Option<Arc<str>>,
}

impl Label {
  pub fn new(id: &str, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      description: None,
    }
  }

  pub fn description(mut self, description: impl Into<Arc<str>>) -> Self {
    self.description = Some(description.into());
    self
  }

  fn render(self) -> Column {
    Column::new()
      .spacing(ExtraSpacing::Fine.role())
      .child(
        Text::new(&self.label)
          .id(format!("{}-label", self.id))
          .variant(ExtraTypography::Strong.role()),
      )
      .with_children(self.description.map(|description| {
        Text::new(&description)
          .id(format!("{}-description", self.id))
          .variant(ExtraTypography::Prose.role())
      }))
  }
}

/// Whether a row is its section's last: the last one draws no line under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
  Within,
  Last,
}

/// The line under a row that is not its section's last.
fn line(place: Place) -> Option<Border> {
  (place == Place::Within).then(|| Border::inside(BorderSize::Sm, PaletteColor::Border))
}

/// The label at the start and `control` at the end, both centered, or at the top with `top`.
pub fn side_row(label: Label, control: impl Into<Element>, place: Place, top: bool) -> Element {
  let id = format!("{}-row", label.id);
  let frame = Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Md))
    .spacing(SpacingSize::Xl)
    .align_items(if top { Alignment::Start } else { Alignment::Center })
    .child(label.render().flex(1.0))
    .child(control);
  match line(place) {
    Some(line) => frame.border_bottom(line).into(),
    None => frame.into(),
  }
}

/// The label over `control`, which runs the row's width.
pub fn stacked_row(label: Label, control: impl Into<Element>, place: Place) -> Element {
  let id = format!("{}-row", label.id);
  let frame = Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .child(label.render())
    .child(control);
  match line(place) {
    Some(line) => frame.border_bottom(line).into(),
    None => frame.into(),
  }
}

/// A feature of the form that is not built yet: its button faded, with a lock, and why under it
/// (`Attach context` `p6caAv`, `Preview` `CGuZo`).
pub fn not_built(id: &str, action: &str, reason: &str) -> Column {
  with_reason(
    ActionButton::new(id, ButtonKind::Secondary, action).unavailable(reason),
    &format!("{id}-reason"),
    reason,
  )
}
