//! Field (`ui::field::field`; `Field/text` `JFXZg`): lurq's `ui::Field` as wide as its parent, its label in 11/500
//! `text_secondary` over the control and its description in 11 `text_muted` under it, 6 apart. lurq's 260 and its
//! label in `text_primary` are not the design's.

use lurq::{
  app::theme::Theme,
  ui::{Field, FieldPart, Length},
};

use crate::theme::ExtraColor;

pub(super) fn install(theme: &Theme) {
  theme.patch_recipe::<Field>(|rules| {
    rules
      .base(FieldPart::Root, |style| style.width(Length::Fill))
      .base(FieldPart::Label, |style| {
        style.foreground(ExtraColor::TextSecondary.role())
      })
  });
}
