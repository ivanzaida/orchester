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
pub use metrics::{ExtraRadius, ExtraSpacing};
pub use palette::ExtraColor;
pub use typography::ExtraTypography;

/// Height of the custom title bar drawn above every screen (`QQmsI`, `V70DN`).
pub const TITLE_BAR_HEIGHT: f32 = tokens::TITLE_BAR_HEIGHT;
/// Width of each Windows window control (`QQmsI`).
pub const WINDOW_CONTROL_WIDTH: f32 = tokens::WINDOW_CONTROL_WIDTH;
/// Size of the chevron lurq draws in a select, the field's disclosure icon (`eASPF` in `ZGwlv`).
pub const SELECT_CHEVRON_SIZE: f32 = tokens::SELECT_CHEVRON_SIZE;

const FONTS: &[&[u8]] = &[
  include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
  include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
  include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
  include_bytes!("../../assets/fonts/Inter-Bold.ttf"),
  include_bytes!("../../assets/fonts/GeistMono-Regular.ttf"),
  // Lucide icons (ISC license), the same font lurq bundles for its DevTools.
  include_bytes!("../../assets/fonts/lucide.ttf"),
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
