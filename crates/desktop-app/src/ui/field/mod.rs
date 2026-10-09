//! Form fields (`Field/text` `JFXZg`, `Field/textarea` `s72p3L`, `Field/search` `Z7mGr2`, `Field/select`
//! `pV2IA`, `Limit field` `UQFos`): a label in 11/500 `text_secondary` over the control, a hint in 11
//! `text_muted` under it, 6 apart. An error is a 12 `circle-alert` and the words in 11.5,
//! both `danger`, and the control's stroke turns `danger` (`CqBFL`).

mod input;
mod number;
mod select;

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

/// `label` over `control` and `hint` under it, as wide as their parent; the hint's element id is
/// `<id>-hint`. A control's error is its own ([`FieldInput::error`]).
pub fn field(id: &str, label: Option<&str>, control: impl Into<Element>, hint: Option<&str>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .with_children(label.map(|label| {
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(TypographyStyle::FieldLabel)
    }))
    .child(control)
    .with_children(hint.map(|hint| {
      Text::new(hint)
        .id(format!("{id}-hint"))
        .variant(TypographyStyle::Caption)
    }))
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
