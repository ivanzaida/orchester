//! The roles of choosing how a subscription takes its target's work (OA-10; board `Orchestrator autonomy · frames to
//! approve` `uYCy4`: `Mode` `GA2bV` in `L03` `xE361`, `ccoKs` and `q5tvtE` in `L02` `HpcQJ`, `Ea9jV` in `Parts ·
//! Ceilings` `H6wvm`).
use orchester_app::theme::{CEILINGS_FORM_WIDTH, LeadingSpacing, LeadingText};

use super::*;

#[test]
fn leading_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line, family, color) in [
    // A way not chosen and the mode on a row (`qkjPe`, `omouE`), what the chosen way means (`R0FlD`), a ceiling's
    // value (`xnfVf`), a field's unit (`lySl8`).
    (LeadingText::Way, 11.5, 1.3, "Geist", PaletteColor::TextSecondary),
    (LeadingText::Meaning, 11.5, 1.45, "Geist", PaletteColor::TextMuted),
    (
      LeadingText::Ceiling,
      11.0,
      1.3,
      "Geist Mono",
      PaletteColor::TextSecondary,
    ),
    (LeadingText::Unit, 12.0, 1.3, "Geist", PaletteColor::TextMuted),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (
        style.font_size,
        style.line_height,
        style.weight,
        &*style.font_family,
        style.color
      ),
      (size, line, FontWeight::Normal, family, theme.palette_color(color)),
      "{role:?}"
    );
  }
  for (spacing, value) in [(LeadingSpacing::WaySide, 9.0), (LeadingSpacing::ControlSide, 7.0)] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
  assert_eq!(CEILINGS_FORM_WIDTH, 440.0, "`Ea9jV`, `ZHAjU`");
}
