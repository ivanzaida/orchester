//! Values of Project settings in its redesign (`Project settings · redesign` `XGtkg` in the new `orchester.pen`): the
//! page's header, its 720 column of groups, a group's heading over its bordered box of rows, and a row's label, hint
//! and value (`Rows` in `V1Fos`, `Name` `Pt6ry`, `Key` `PYS3f`).
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectSettingsText {
  /// 12/600 `text_secondary`: a group's heading over its box (`Title` `W1qK7`).
  GroupTitle,
  /// 12 `text_muted`: a row's one hint line under its label (`Hint` `A9HnPm`); a hint in a tone takes its color.
  Hint,
  /// 12.5 `text_secondary`: a read-only value at a row's end (`Text` `IxKWi`).
  Value,
  /// Geist Mono 12.5/500 `text_primary`: an identifier as a row's label, a secret's name (`Name` `HaAqu`).
  MonoLabel,
  /// 16/600 `text_primary`: a sub-page's or an opened item's title (`Title` `RYStU`, `N9Na8Q`).
  PageTitle,
}

impl ProjectSettingsText {
  pub const ALL: [Self; 5] = [
    Self::GroupTitle,
    Self::Hint,
    Self::Value,
    Self::MonoLabel,
    Self::PageTitle,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::GroupTitle => "project_settings_group_title",
      Self::Hint => "project_settings_hint",
      Self::Value => "project_settings_value",
      Self::MonoLabel => "project_settings_mono_label",
      Self::PageTitle => "project_settings_page_title",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::GroupTitle => style(12.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_secondary),
      Self::Hint => style(12.0, FontWeight::Normal, 1.35, 0.0, palette.text_muted),
      Self::Value => style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::MonoLabel => mono(style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary)),
      Self::PageTitle => style(16.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectSettingsSpacing {
  /// 24: above the title in the content panel (`Header` `N4G7h`).
  HeaderTop,
  /// 28: the page's sides, its header's, its tabs' and its column's (`N4G7h`, `A5Sf0Q`, `mBTu6`).
  Side,
  /// 22: above the column under the tabs, the body's 20 and the pane's 2 (`mBTu6`, `YsyE9`).
  BodyTop,
  /// 11: over and under a row's words (`Pt6ry`).
  RowY,
  /// 14: a row's sides (`Pt6ry`).
  RowX,
}

impl ProjectSettingsSpacing {
  pub const ALL: [Self; 5] = [Self::HeaderTop, Self::Side, Self::BodyTop, Self::RowY, Self::RowX];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::HeaderTop => "project_settings_header_top",
      Self::Side => "project_settings_side",
      Self::BodyTop => "project_settings_body_top",
      Self::RowY => "project_settings_row_y",
      Self::RowX => "project_settings_row_x",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::HeaderTop => 24.0,
      Self::Side => 28.0,
      Self::BodyTop => 22.0,
      Self::RowY => 11.0,
      Self::RowX => 14.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in ProjectSettingsSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ProjectSettingsText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
