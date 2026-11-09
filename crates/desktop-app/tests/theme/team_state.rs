//! A team task's state type roles (`Task - v2` `jDN0r`, T11a `ujJ6U`).
use orchester_app::theme::TeamStateText;

use super::*;

#[test]
fn team_state_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line, weight, color) in [
    (
      TeamStateText::Title,
      15.0,
      1.35,
      FontWeight::SemiBold,
      PaletteColor::TextPrimary,
    ),
    (
      TeamStateText::Detail,
      12.0,
      1.5,
      FontWeight::Normal,
      PaletteColor::TextSecondary,
    ),
    (
      TeamStateText::Reason,
      12.0,
      1.5,
      FontWeight::Normal,
      PaletteColor::TextPrimary,
    ),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.line_height, style.weight, &*style.font_family),
      (size, line, weight, "Geist"),
      "{role:?}"
    );
    assert_eq!(style.color, theme.palette_color(color), "{role:?}");
  }
}
