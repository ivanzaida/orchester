//! Key cap (`Kbd` `wMPlq`): the keys of a shortcut in Geist Mono 10 `text_secondary`, 5 in from the sides of an 18
//! high cap on `surface_raised` in a `border` stroke, radius 4.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::theme::{ExtraSpacing, ExtraTypography};

/// The cap's height (`wMPlq`).
const HEIGHT: f32 = 18.0;

/// A cap with `keys`; `id` names it.
pub fn key_cap(id: &str, keys: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .height(Dimension::Px(HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Pair.role()))
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Sm)
    .child(Text::new(keys).variant(ExtraTypography::MonoSmall.role()).nowrap())
    .into()
}
