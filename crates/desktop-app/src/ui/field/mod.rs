//! Form fields (`Field/text` `JFXZg`, `Field/textarea` `s72p3L`, `Field/search` `Z7mGr2`, `Field/select`
//! `pV2IA`, `Limit field` `UQFos`): a label in 11/500 `text_secondary` over the control, a hint in 11
//! `text_muted` under it, 6 apart. An error is a 12 `circle-alert` and the words in 11.5,
//! both `danger`, and the control's stroke turns `danger` (`CqBFL`).

mod input;
mod number;
mod select;

use lurq::{
  app::theme::PaletteColor,
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
  ui::Field,
};

pub use self::{
  input::{FieldInput, UnitLook},
  number::NumberField,
  select::{SelectField, select_style},
};
use super::icon::{Icon, icon};
use crate::theme::{ExtraSpacing, ExtraTypography, IconSize};

/// lurq's `ui::Field` (`theme::kit::field`): `label` over `control` and `hint` under it, as wide as their parent; the
/// label's element id is `<id>-label` and the hint's `<id>-description`. A control's error is its own
/// ([`FieldInput::error`]); what the caller puts under the field goes in [`Field::below`].
pub fn field(id: &str, label: Option<&str>, control: impl Into<Element>, hint: Option<&str>) -> Field {
  let control: Element = control.into();
  let field = match label {
    Some(label) => Field::new(label, control),
    None => Field::unlabeled(control),
  }
  .id(id.to_owned());
  match hint {
    Some(hint) => field.description(hint),
    None => field,
  }
}

/// Why a value is not accepted (`Field error` `d5xipm`), under the field or action it is about. It is as
/// wide as its parent and its words wrap there, the mark centred on them.
pub fn field_error(id: &str, text: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("role", "field-error")
    .child(icon(Icon::CircleAlert, IconSize::Sm, PaletteColor::Destructive).flex_shrink(0.0))
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::Destructive)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}
