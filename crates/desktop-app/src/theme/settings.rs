//! The design values of the app's Settings (`Settings · Automation · dark` `oD7hy` in the Settings layer
//! `uyO5v`; the redesign's board `App settings · redesign` `iUmo3` in the new `orchester.pen`) that the UI kit's
//! roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// Text of Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsText {
  /// 13/600 `text_primary`: a form section's title (`Form section title` `De1nE`), a step's title (`DwjDQ` in
  /// `MrVql`).
  SectionTitle,
  /// 13 `text_secondary`: what Settings holds, under its title (`Sub` `mQJGo` in `t098v`).
  Lead,
  /// 12/600 `text_secondary`: a group's heading over its rows (`Title` `l8uoE2` in `l838Ty`).
  GroupTitle,
  /// 12.5 `text_muted`: what Settings is, beside its title (`Sub` `Uq8Sh` in `l838Ty`).
  PageSub,
  /// 16/600 `text_primary`: a sub-page's title (`Title` `e7z5H` in `xN1Jy`).
  SubTitle,
  /// 14/600 `text_primary`: the cloud account's name (`Name` `dNkSD` in `vPV73`).
  CardName,
  /// 12/600 `text_primary`: the initials in the cloud account's avatar (`Initials` `QkgEZ`).
  CardInitials,
  /// Geist Mono 16/500 `text_primary`: a certificate's check code (`Value` `UWPnK` in `IBnMu`).
  Code,
  /// Geist Mono 18/500 `text_primary`: a pairing code (`Code` `M6bqk` in `ZUuMT`).
  PairingCode,
}

impl SettingsText {
  pub const ALL: [Self; 9] = [
    Self::SectionTitle,
    Self::Lead,
    Self::GroupTitle,
    Self::PageSub,
    Self::SubTitle,
    Self::CardName,
    Self::CardInitials,
    Self::Code,
    Self::PairingCode,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::SectionTitle => "settings_section_title",
      Self::Lead => "settings_lead",
      Self::GroupTitle => "settings_group_title",
      Self::PageSub => "settings_page_sub",
      Self::SubTitle => "settings_sub_title",
      Self::CardName => "settings_card_name",
      Self::CardInitials => "settings_card_initials",
      Self::Code => "settings_code",
      Self::PairingCode => "settings_pairing_code",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::SectionTitle => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Lead => style(13.0, FontWeight::Normal, 1.4, 0.0, palette.text_secondary),
      Self::GroupTitle => style(12.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_secondary),
      Self::PageSub => style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::SubTitle => style(16.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::CardName => style(14.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::CardInitials => style(12.0, FontWeight::SemiBold, 1.0, 0.0, palette.text_primary),
      Self::Code => mono(style(16.0, FontWeight::Medium, 1.3, 0.0, palette.text_primary)),
      Self::PairingCode => mono(style(18.0, FontWeight::Medium, 1.3, 0.0, palette.text_primary)),
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
  /// 11: over and under a row of a group (`Theme` `IcZES` in `l838Ty`).
  RowY,
  /// 28: the page's sides, its header's, its tabs' and its column's (`Header` `t7cJ0W`, `Tabs` `xEFQ0`, `Body`
  /// `EUrDy`), and between its groups (`Pane` `ZbG8J`).
  Page,
  /// 20: above the column under the tabs (`Body` `EUrDy`).
  BodyTop,
  /// 40: a row that depends on the one above it starts this far in (`Merge finished runs on its own` `JMLwI`).
  Dependent,
}

impl SettingsSpacing {
  pub const ALL: [Self; 7] = [
    Self::PanelTop,
    Self::PanelSide,
    Self::SectionTitle,
    Self::RowY,
    Self::Page,
    Self::BodyTop,
    Self::Dependent,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::PanelTop => "settings_panel_top",
      Self::PanelSide => "settings_panel_side",
      Self::SectionTitle => "settings_section_title",
      Self::RowY => "settings_row_y",
      Self::Page => "settings_page",
      Self::BodyTop => "settings_body_top",
      Self::Dependent => "settings_dependent",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::PanelTop | Self::Page => 28.0,
      Self::PanelSide | Self::Dependent => 40.0,
      Self::SectionTitle => 10.0,
      Self::RowY => 11.0,
      Self::BodyTop => 20.0,
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
