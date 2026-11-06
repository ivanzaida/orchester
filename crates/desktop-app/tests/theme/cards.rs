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

#[test]
fn cards_model_approved_roles_match_06() {
  for mode in [Mode::Dark, Mode::Light] {
    let app = themed(mode);
    let theme = app.theme();
    for (role, color) in [
      (CardsText::AnsweredCommand, PaletteColor::TextPrimary),
      (CardsText::RiskValue, PaletteColor::TextSecondary),
    ] {
      let style = theme.typography_style(role.role());
      assert_eq!(style.font_size, 12.0);
      assert_eq!(style.line_height, 1.3);
      assert_eq!(style.color, theme.palette_color(color));
    }
    assert_eq!(orchester_app::theme::RISK_TRACK_HEIGHT, 4.0);
  }
}
