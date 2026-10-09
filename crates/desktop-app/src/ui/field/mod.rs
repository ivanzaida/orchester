//! Form fields (`Field/text` `JFXZg`, `Field/textarea` `s72p3L`, `Field/search` `Z7mGr2`, `Field/select`
//! `pV2IA`, `Limit field` `UQFos`): a label in 11/500 `text_secondary` over the control, a hint in 11
//! `text_muted` or an error under it, 6 apart. An error is a 12 `circle-alert` and the words in 11.5,
//! both `danger`, and the control's stroke turns `danger` (`CqBFL`).

mod input;
mod number;
mod select;

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, TypographyStyle},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

pub use self::{
  input::FieldInput,
  number::NumberField,
  select::{SelectField, select_style},
};
use super::icon::{Icon, icon};
use crate::theme::{ExtraSpacing, ExtraTypography, IconSize};

/// What a field says under its control.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FieldNote {
  #[default]
  None,
  /// How to fill it in.
  Hint(Arc<str>),
  /// Why its value is not accepted.
  Error(Arc<str>),
}

/// `label` over `control` and `note` under it, as wide as their parent; the note's element id is
/// `<id>-note`.
pub fn field(id: &str, label: Option<&str>, control: impl Into<Element>, note: &FieldNote) -> Column {
  let mut column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .with_children(label.map(|label| {
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(TypographyStyle::FieldLabel)
    }))
    .child(control);
  let note_id = format!("{id}-note");
  match note {
    FieldNote::None => {}
    FieldNote::Hint(hint) => column = column.child(Text::new(hint).id(note_id).variant(TypographyStyle::Caption)),
    FieldNote::Error(error) => column = column.child(field_error(&note_id, error)),
  }
  column
}

/// Why a value is not accepted (`Field error` `d5xipm`), under the field or action it is about.
pub fn field_error(id: &str, text: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("role", "field-error")
    .child(icon(Icon::CircleAlert, IconSize::Sm, PaletteColor::Danger))
    .child(
      Text::new(text)
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::Danger),
    )
}
