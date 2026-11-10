//! The roles of the step strip on an open step (`Step strip` in `Pipelines · v2` P2 `u2rYEH`, P13 `KRQ5J`).

use lurq::{app::theme::PaletteColor, node::dimension::Dimension};
use orchester_app::theme::{StripSpacing, StripText, chip_radius};

use super::*;

#[test]
fn strip_text_follows_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line_height, color, family) in [
    (StripText::Number, 11.0, 1.2, PaletteColor::TextMuted, "Geist Mono"),
    (StripText::Name, 12.0, 1.2, PaletteColor::TextSecondary, "Geist"),
    (StripText::Current, 12.0, 1.2, PaletteColor::TextPrimary, "Geist"),
    // The gate's hand (`Gate` in `Step strip`), a 9 glyph of the icon font.
    (StripText::GateHand, 9.0, 1.0, PaletteColor::Warning, "lucide"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.line_height, style.color, &*style.font_family),
      (size, line_height, theme.palette_color(color), family),
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
