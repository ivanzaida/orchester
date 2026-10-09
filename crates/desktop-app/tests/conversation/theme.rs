//! The conversation's own theme roles resolve to the `Chat` components of `data/orchester2.pen` (`wtGg8`).

use lurq::{
  app::{App, theme::TypographyStyle},
  layout::text_style::FontWeight,
  node::{color::Color, dimension::Dimension},
};
use orchester_app::theme::{self, ConversationRadius, ConversationSpacing, ConversationText, Mode};

#[test]
fn timeline_theme_roles_follow_the_chat_components() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  let check = |role: TypographyStyle, (size, line_height): (f32, f32), weight: FontWeight, color: &str| {
    let style = theme.typography_style(role);
    assert_eq!(
      (style.font_size, style.line_height, style.weight, style.color),
      (size, line_height, weight, Color::from_hex(color)),
      "{role:?}"
    );
  };
  check(
    ConversationText::Name.role(),
    (12.0, 1.35),
    FontWeight::SemiBold,
    "#EDEDF0",
  );
  check(
    ConversationText::Divider.role(),
    (11.0, 1.4),
    FontWeight::Medium,
    "#8A8A92",
  );
  check(
    ConversationText::ProblemDetail.role(),
    (11.5, 1.45),
    FontWeight::Normal,
    "#B3B3BA",
  );
  check(
    ConversationText::ResultTitle.role(),
    (12.5, 1.35),
    FontWeight::SemiBold,
    "#EDEDF0",
  );
  check(
    ConversationText::StateTitle.role(),
    (13.5, 1.35),
    FontWeight::SemiBold,
    "#EDEDF0",
  );
  for (spacing, value) in [
    (ConversationSpacing::ToolRows, 20.0),
    (ConversationSpacing::Mark, 29.0),
    (ConversationSpacing::ProblemRow, 9.0),
    (ConversationSpacing::StateChip, 9.0),
  ] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
  assert_eq!(theme.radius_value(ConversationRadius::ProblemRow.role()), 8.0);
}
