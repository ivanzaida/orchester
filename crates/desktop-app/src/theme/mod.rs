//! Orchester's lurq theme, from the variables and type roles of `data/orchester2.pen` (Theme board
//! `w3uvVL`), with the `graphite` accent in dark and light mode.
//!
//! UI code styles only through lurq roles (`PaletteColor`, `TypographyStyle`, `SpacingSize`,
//! `RadiusSize`, `BorderSize`, `ShadowStyle`) and the extra roles defined here; design values live only
//! in this module.

mod fonts;
mod form;
mod metrics;
mod palette;
mod shadow;
mod typography;

use lurq::{
  app::{App, theme::Theme},
  layout::scrollbar::ScrollBarStyle,
};

pub use self::{
  metrics::{ExtraRadius, ExtraSpacing},
  palette::{ExtraColor, Mode},
  typography::{ExtraTypography, IconSize},
};

/// Loads the fonts and fills every role for `mode`.
pub fn install(app: &mut App, mode: Mode) {
  fonts::install(app);
  let theme = app.theme();
  apply(theme, mode);
  metrics::install(theme);
  theme.set_shadows(shadow::shadows());
  theme.set_form(form::form());
}

/// Switches the colors to `mode`: the palette, and the text styles and scrollbar whose colors come
/// from it. Every component that reads the theme follows on its next render.
pub fn apply(theme: &Theme, mode: Mode) {
  let palette = palette::palette(mode);
  theme.set_typography(typography::typography(&palette));
  theme.set_scrollbar(ScrollBarStyle {
    thumb_color: palette.border_strong,
    ..ScrollBarStyle::thin()
  });
  theme.set_palette(palette);
}
