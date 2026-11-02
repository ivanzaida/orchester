//! 06b denial editor type (cxLoV).
use orchester_app::theme::CardsText;

use super::*;

#[test]
fn cards_denial_message_role_matches_06b() {
  for mode in [Mode::Dark, Mode::Light] {
    let app = themed(mode);
    let theme = app.theme();
    let style = theme.typography_style(CardsText::DenialMessage.role());
    assert_eq!(style.font_size, 13.0);
    assert_eq!(style.line_height, 1.5);
    assert_eq!(style.weight, FontWeight::Normal);
    assert_eq!(style.color, theme.palette_color(PaletteColor::TextPrimary));
  }
}
