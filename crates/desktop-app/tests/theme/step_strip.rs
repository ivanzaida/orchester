//! The roles of the step strip on an open step (`Step strip` in `Pipelines · v2` P2 `u2rYEH`, P13 `KRQ5J`).

use lurq::{app::theme::PaletteColor, node::dimension::Dimension};
use orchester_app::theme::{StripColor, StripSpacing, StripText, chip_radius};

use super::*;

#[test]
fn strip_text_follows_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, color, family) in [
    (StripText::Number, 11.0, PaletteColor::TextMuted, "Geist Mono"),
    (StripText::Name, 12.0, PaletteColor::TextSecondary, "Geist"),
    (StripText::Current, 12.0, PaletteColor::TextPrimary, "Geist"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.line_height, style.color, &*style.font_family),
      (size, 1.2, theme.palette_color(color), family),
      "{role:?}"
    );
  }
}

#[test]
fn a_chip_has_its_own_sides_and_corners() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  assert_eq!(theme.spacing_value(StripSpacing::ChipSide.role()), Dimension::Px(7.0));
  assert_eq!(theme.radius_value(chip_radius()), 4.0);
}

/// The fade under ‹ and › ends in `surface_raised` fully transparent, so it does not grey the chips it hides.
#[test]
fn the_fade_ends_in_the_strips_own_color_made_clear() {
  for mode in [Mode::Dark, Mode::Light] {
    let app = themed(mode);
    let theme = app.theme();
    let (raised, clear) = (
      theme.palette_color(PaletteColor::SurfaceRaised),
      theme.palette_color(StripColor::FadeClear.role()),
    );
    assert_eq!(clear, raised.with_alpha(0), "{mode:?}");
  }
}
