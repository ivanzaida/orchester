//! The cards' own theme roles resolve to `Question card` `tKYgO`, `Plan card` `Q5J0r` and `Approval card`
//! `gwOaJ` of `data/orchester2.pen`.

use lurq::{
  app::App,
  layout::text_style::FontWeight,
  node::{color::Color, dimension::Dimension},
};
use orchester_app::theme::{self, CardsSpacing, CardsText, Mode};

#[test]
fn card_theme_roles_follow_the_card_components() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  for (role, (size, line_height), weight, color) in [
    (CardsText::Title, (12.5, 1.35), FontWeight::SemiBold, "#EDEDF0"),
    (CardsText::ApprovalTitle, (13.0, 1.35), FontWeight::SemiBold, "#EDEDF0"),
    (CardsText::PlanLines, (12.5, 1.6), FontWeight::Normal, "#B3B3BA"),
    (CardsText::Decided, (12.5, 1.35), FontWeight::Medium, "#EDEDF0"),
    (CardsText::Action, (12.5, 1.5), FontWeight::Normal, "#B3B3BA"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.line_height, style.weight, style.color),
      (size, line_height, weight, Color::from_hex(color)),
      "{role:?}"
    );
  }
  assert_eq!(theme.spacing_value(CardsSpacing::PlanHead.role()), Dimension::Px(11.0));
}
