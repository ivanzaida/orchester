//! The Memory screens' type and spacing roles (`J1utD`, `CJvK7`, `Q0BAI`, `sglU2`, `xY2Uj`, `n3lxin`, `X0l2I6`,
//! `xFUjH`).

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

/// What a record says in the Agents hub's panel (`i5mAK` in Y1 `tsfxN`): 13 `text_primary`, 1.5.
#[test]
fn memory_theme_record_text_resolves_the_design() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  let style = theme.typography_style(MemoryText::Record.role());
  assert_eq!(
    (style.font_size, style.weight, &*style.font_family),
    (13.0, FontWeight::Normal, "Geist")
  );
  assert_eq!(style.color, theme.palette_color(PaletteColor::TextPrimary));
}

/// A status chip's word in the Agents hub's panel (`H3ItbS` in Y1 `tsfxN`): 11/500, 1.2, so the chip is 19 high.
#[test]
fn memory_theme_chip_text_resolves_the_design() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let style = app.theme().typography_style(MemoryText::Chip.role());
  assert_eq!((style.font_size, style.weight), (11.0, FontWeight::Medium));
  assert!((style.line_height - 1.2).abs() < f32::EPSILON);
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
