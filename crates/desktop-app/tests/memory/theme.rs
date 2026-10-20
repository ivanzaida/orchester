//! The Memory screens' type and spacing roles (`J1utD`, `CJvK7`, `Q0BAI`, `sglU2`, `xY2Uj`, `n3lxin`, `X0l2I6`).

use lurq::{
  app::{App, theme::PaletteColor},
  layout::text_style::FontWeight,
  node::dimension::Dimension,
};
use orchester_app::theme::{self, MemorySpacing, MemoryText, Mode};

#[test]
fn memory_theme_roles_resolve_the_design() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  let expected = [
    (
      MemoryText::GroupLabel,
      10.5,
      FontWeight::Medium,
      "Geist",
      PaletteColor::TextMuted,
    ),
    (
      MemoryText::GroupCount,
      10.5,
      FontWeight::Normal,
      "Geist Mono",
      PaletteColor::TextMuted,
    ),
    (
      MemoryText::Flag,
      10.5,
      FontWeight::Medium,
      "Geist",
      PaletteColor::TextSecondary,
    ),
    (
      MemoryText::ListLabel,
      11.5,
      FontWeight::Medium,
      "Geist",
      PaletteColor::TextMuted,
    ),
    (
      MemoryText::NoticeText,
      11.5,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextSecondary,
    ),
    (
      MemoryText::Scope,
      10.5,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextMuted,
    ),
    (
      MemoryText::Lead,
      13.0,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextSecondary,
    ),
    (
      MemoryText::PanelTitle,
      14.0,
      FontWeight::SemiBold,
      "Geist",
      PaletteColor::TextPrimary,
    ),
    (
      MemoryText::StateText,
      12.0,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextSecondary,
    ),
  ];
  for (role, size, weight, family, color) in expected {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family),
      (size, weight, family),
      "{role:?}"
    );
    assert_eq!(style.color, theme.palette_color(color), "{role:?}");
  }
}

#[test]
fn memory_theme_group_label_tracking_and_spacing_resolve_the_design() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  assert_eq!(
    theme.typography_style(MemoryText::GroupLabel.role()).letter_spacing,
    0.5
  );
  for (role, value) in [
    (MemorySpacing::Groups, 18.0),
    (MemorySpacing::TextIndent, 28.0),
    (MemorySpacing::ListSide, 20.0),
    (MemorySpacing::RowText, 40.0),
    (MemorySpacing::StateTop, 36.0),
  ] {
    assert_eq!(theme.spacing_value(role.role()), Dimension::Px(value), "{role:?}");
  }
}
