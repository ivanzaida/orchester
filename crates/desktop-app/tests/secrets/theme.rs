//! The Secrets layer's type and spacing roles (ccz8n).
use lurq::{
  app::{App, theme::PaletteColor},
  layout::text_style::FontWeight,
  node::dimension::Dimension,
};
use orchester_app::theme::{self, Mode, SecretsSpacing, SecretsText};

#[test]
fn secrets_theme_roles_resolve_the_wide_dark_frames() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  let expected = [
    (
      SecretsText::Lead,
      13.0,
      1.4,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextSecondary,
    ),
    (
      SecretsText::Name,
      12.5,
      1.4,
      FontWeight::Medium,
      "Geist Mono",
      PaletteColor::TextPrimary,
    ),
    (
      SecretsText::Mask,
      14.0,
      1.3,
      FontWeight::Normal,
      "Geist Mono",
      PaletteColor::TextSecondary,
    ),
    (
      SecretsText::InspectorTitle,
      14.0,
      1.3,
      FontWeight::SemiBold,
      "Geist",
      PaletteColor::TextPrimary,
    ),
    (
      SecretsText::ColumnHeading,
      11.0,
      1.3,
      FontWeight::Medium,
      "Geist",
      PaletteColor::TextMuted,
    ),
    (
      SecretsText::ConfirmTitle,
      12.5,
      1.4,
      FontWeight::Medium,
      "Geist",
      PaletteColor::TextPrimary,
    ),
  ];
  for (role, size, height, weight, family, color) in expected {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.line_height, style.weight, &*style.font_family),
      (size, height, weight, family),
      "{role:?}"
    );
    assert_eq!(style.color, theme.palette_color(color), "{role:?}");
  }
  for (role, value) in [
    (SecretsSpacing::Page, 40.0),
    (SecretsSpacing::Cell, 12.0),
    (SecretsSpacing::Body, 14.0),
  ] {
    assert_eq!(theme.spacing_value(role.role()), Dimension::Px(value), "{role:?}");
  }
}
