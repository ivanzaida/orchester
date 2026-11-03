//! Frame 02's test lead D9rOj and table cell R5m5p.
use orchester_app::theme::ConnectionsText;

use super::*;

#[test]
fn decision_test_roles_follow_frame_02() {
  for mode in [Mode::Dark, Mode::Light] {
    let app = themed(mode);
    let theme = app.theme();
    for (role, size, height, color) in [
      (ConnectionsText::TestLead, 12.0, 1.45, PaletteColor::TextMuted),
      (ConnectionsText::TestCell, 12.0, 1.3, PaletteColor::TextSecondary),
    ] {
      let style = theme.typography_style(role.role());
      assert_eq!((style.font_size, style.line_height), (size, height));
      assert_eq!(style.color, theme.palette_color(color));
    }
  }
}
