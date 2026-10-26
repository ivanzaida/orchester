//! Words the tab's parts share: what a part says in 12.5 `text_secondary` (`ysTEY`) and a hint under its actions in
//! 11.5 `text_muted` (`FVLQK`), both wrapping.

use lurq::{
  components::Text,
  node::dimension::Dimension,
};

use crate::theme::ExtraTypography;

/// What a part says: 12.5 `text_secondary`, wrapping.
pub(super) fn paragraph(id: &str, text: &str) -> Text {
  Text::new(text)
    .id(id.to_owned())
    .variant(ExtraTypography::Lead.role())
    .width(Dimension::Pct(100.0))
}

/// A hint under a part's actions: 11.5 `text_muted`, wrapping.
pub(super) fn hint(id: &str, text: &str) -> Text {
  Text::new(text)
    .id(id.to_owned())
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
}
