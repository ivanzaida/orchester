//! Key cap (`Kbd` `wMPlq`): the keys of a shortcut in Geist Mono 10 `text_secondary`, 5 in from the sides of an 18
//! high cap on `surface_raised` in a `border` stroke, radius 4.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, Theme},
  node::border::Border,
  ui::{Kbd, KbdPart},
};

use super::plain::plain;
use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography};

/// The cap's height (`wMPlq`).
const HEIGHT: f32 = 18.0;
/// `MonoSmall`'s size and line, over lurq's own label size.
const LABEL_SIZE: f32 = 10.0;
const LABEL_LINE: f32 = 1.4;

pub(super) fn install(theme: &Theme) {
  theme.patch_recipe::<Kbd>(|rules| {
    rules
      .base(KbdPart::Key, |style| {
        style
          .height(HEIGHT)
          .padding_x(ExtraSpacing::Pair.role())
          .radius(RadiusSize::Sm)
          .background(PaletteColor::Card)
          .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      })
      .base(KbdPart::Label, |style| {
        style
          .typography(plain(ExtraTypography::MonoSmall.role()))
          .font_size(LABEL_SIZE)
          .line_height(LABEL_LINE)
          .foreground(ExtraColor::TextSecondary.role())
      })
  });
}
