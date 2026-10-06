//! The scroll gutter (design doc "Scroll gutter"): a viewport whose content can outgrow its panel
//! reserves a gutter of its own at its right edge (`theme::gutter_scrollbar`), so the thumb never
//! covers a field, a row or its selection. The gutter takes its width from the panel's right
//! padding, or from the gap after the column, so the scrolled content still ends at the designed
//! inset.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};

use crate::theme::gutter_width;

/// `space`, a panel's right padding or the gap after a column, less the scroll gutter put in it.
pub fn beside_gutter(ctx: &Ctx, space: impl Into<SpacingSize>) -> Dimension {
  Dimension::Px(ctx.theme().spacing_value(space).resolve(0.0) - gutter_width())
}

/// `element`, which does not scroll, kept out of the gutter's column, so it ends where the scrolled
/// content does.
pub fn clear_of_gutter(element: impl Into<Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_right(Dimension::Px(gutter_width()))
    .child(element)
    .into()
}
