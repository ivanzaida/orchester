//! The design values of the app's Settings (`Settings · Automation · dark` `oD7hy` in the Settings layer
//! `uyO5v`) that the UI kit's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// Text of Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsText {
  /// 13/600 `text_primary`: a form section's title (`Form section title` `De1nE`).
  SectionTitle,
}

impl SettingsText {
  pub const ALL: [Self; 1] = [Self::SectionTitle];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::SectionTitle => "settings_section_title",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::SectionTitle => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
    }
  }
}

/// Gaps and paddings of Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSpacing {
  /// 28: above the title inside the content panel (`Content · Settings` `kdqHp`).
  PanelTop,
  /// 40: the content panel's sides (`kdqHp`).
  PanelSide,
  /// 10: under a form section's title, above its line (`De1nE`).
  SectionTitle,
}

impl SettingsSpacing {
  pub const ALL: [Self; 3] = [Self::PanelTop, Self::PanelSide, Self::SectionTitle];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::PanelTop => "settings_panel_top",
      Self::PanelSide => "settings_panel_side",
      Self::SectionTitle => "settings_section_title",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::PanelTop => 28.0,
      Self::PanelSide => 40.0,
      Self::SectionTitle => 10.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  SettingsText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in SettingsSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
