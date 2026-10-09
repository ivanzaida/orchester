use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, ThemeTypography, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
  node::color::Color,
};

use super::{
  ExtraColor,
  fonts::{ICONS, MONO, SANS},
};

/// Design text styles without a built-in lurq role, registered as `TypographyStyle::Extra`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraTypography {
  /// `type/display` 28/600, tracking -0.5: the largest headline of a screen.
  Display,
}

impl ExtraTypography {
  pub const ALL: [Self; 1] = [Self::Display];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Display => "display",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Display => style(28.0, FontWeight::SemiBold, 1.15, -0.5, palette.text_primary),
    }
  }
}

/// Glyph sizes of the lucide icon font: the sizes of the design's `icon` nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconSize {
  /// 12: chevrons, a tab's close, status bar icons.
  Sm,
  /// 13: a tab's page icon, the project switcher's chevrons.
  Base,
  /// 14: the All projects tab, new tab, window controls, menu items.
  Md,
  /// 15: navigation items.
  Nav,
  /// 16: the glyph of an empty or unbuilt screen.
  Lg,
}

impl IconSize {
  pub const ALL: [Self; 5] = [Self::Sm, Self::Base, Self::Md, Self::Nav, Self::Lg];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Sm => "icon_sm",
      Self::Base => "icon_base",
      Self::Md => "icon_md",
      Self::Nav => "icon_nav",
      Self::Lg => "icon_lg",
    }
  }

  pub const fn size(self) -> f32 {
    match self {
      Self::Sm => 12.0,
      Self::Base => 13.0,
      Self::Md => 14.0,
      Self::Nav => 15.0,
      Self::Lg => 16.0,
    }
  }
}

/// The type roles of the Theme board (`w3uvVL`): the dense desktop scale, body 13.
pub fn typography(palette: &ThemePalette) -> ThemeTypography {
  let primary = palette.text_primary;
  let extras = ExtraTypography::ALL
    .into_iter()
    .map(|extra| (Arc::from(extra.name()), extra.style(palette)));
  let icons = IconSize::ALL.into_iter().map(|size| {
    let glyph = TextStyle {
      font_family: Arc::from(ICONS),
      ..style(size.size(), FontWeight::Normal, 1.0, 0.0, palette.text_secondary)
    };
    (Arc::from(size.name()), glyph)
  });
  ThemeTypography {
    heading: style(20.0, FontWeight::SemiBold, 1.25, -0.2, primary),
    title: style(15.0, FontWeight::SemiBold, 1.35, -0.1, primary),
    body: style(13.0, FontWeight::Normal, 1.5, 0.0, primary),
    description: style(12.0, FontWeight::Normal, 1.5, 0.0, palette.text_secondary),
    caption: style(11.0, FontWeight::Normal, 1.4, 0.0, palette.text_muted),
    label: style(12.0, FontWeight::Medium, 1.35, 0.0, primary),
    field_label: style(11.0, FontWeight::Medium, 1.35, 0.0, palette.text_secondary),
    button: style(12.0, FontWeight::Medium, 1.2, 0.0, primary),
    link: style(
      13.0,
      FontWeight::Normal,
      1.5,
      0.0,
      palette.get(ExtraColor::AccentText.role()),
    ),
    mono: TextStyle {
      font_family: Arc::from(MONO),
      ..style(12.0, FontWeight::Normal, 1.5, 0.0, primary)
    },
    extra: extras
      .chain(icons)
      .chain(super::startup::typography(palette))
      .chain(super::shell::typography(palette))
      .collect(),
  }
}

pub(super) fn style(
  font_size: f32,
  weight: FontWeight,
  line_height: f32,
  letter_spacing: f32,
  color: Color,
) -> TextStyle {
  TextStyle {
    font_family: Arc::from(SANS),
    font_size,
    weight,
    line_height,
    letter_spacing,
    color,
    ..TextStyle::default()
  }
}
