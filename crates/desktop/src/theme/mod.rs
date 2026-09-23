//! Orchester's lurq theme, built from the Pencil design variables.
//!
//! UI code styles through lurq roles (`PaletteColor`, `TypographyStyle`, `SpacingSize`,
//! `RadiusSize`, `BorderSize`); design values live only in `tokens`.

mod form;
mod metrics;
mod palette;
mod tokens;
mod typography;

use lurq::app::App;
pub use palette::ExtraColor;
pub use typography::ExtraTypography;

const FONTS: &[&[u8]] = &[
  include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
  include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
  include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
  include_bytes!("../../assets/fonts/Inter-Bold.ttf"),
];

pub fn install(app: &mut App) {
  for font in FONTS {
    app.load_font(font.to_vec());
  }

  let theme = app.theme();
  theme.set_palette(palette::palette());
  theme.set_typography(typography::typography());
  theme.set_radii(metrics::radii());
  theme.set_spacing(metrics::spacing());
  theme.set_border_sizes(metrics::border_sizes());
  // Bind first: the guard returned by `form()` must be released before `set_form` locks.
  let current_form = theme.form().clone();
  theme.set_form(form::form(current_form));
}
