//! The design values of the startup screens (`Startup · Orchester is already open` `g0Kj1`, `Startup ·
//! Orchester can't open its data` `kMM56` and the variants board `ut8NI`) and of the start page's steps (`Startup ·
//! redesign` `qzzYw`: *Join a team* `B914L`, `drCQd`) that the UI kit's roles do not carry, registered as lurq
//! `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::{
  fonts::ICONS,
  typography::{mono, style},
};

/// Text of the startup screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupText {
  /// The 20 glyph of the icon tile (`Glyph` `Br0GY`).
  TileIcon,
  /// 12.5 `text_muted`, line 16: a crumb before the step's own, *Start* (`Ancestor` in `LTKnD`).
  Crumb,
  /// 12.5/500 `text_primary`, line 16: the step's own crumb (`Current` in `LTKnD`).
  CrumbCurrent,
  /// 10.5/500 `text_primary`, line 13: a step's number in its ring (`P6t15`); a step not reached in `text_muted`.
  StepNumber,
  /// 12 `text_muted`, line 16: a step not reached in the step line (`F7crC`).
  StepLabel,
  /// 12/500 `text_primary`, line 16: the current step in the step line.
  StepLabelCurrent,
  /// 22/600 `text_primary`, line 28, tracking -0.4: a step's title (`Title` in `bKHEo`).
  Title,
  /// 13.5 `text_secondary`, line 21: what a step is, under its title (`Lead` in `bKHEo`).
  Lead,
  /// 15/600 `text_primary`, line 20: the title of a way in, *With a code* (`Title` in `ER5j6`).
  WayTitle,
  /// 12 `text_muted`, line 17: what a way in is for, under its title, and the note under the step (`n9aRSG`).
  Sub,
  /// 12.5 `text_muted`, line 17: *No account yet?* and *· signed in with Orchester Cloud* (`uNGRZ`).
  Aside,
  /// 12.5/500 `text_primary`, line 17: a link after an aside, *Create one ↗*.
  Link,
  /// 11/500 `text_muted`, line 14, tracking 0.4: a section's heading, *YOUR TEAMS* (`allZn`).
  Section,
  /// 13.5/500 `text_primary`, line 18: a team's or an invite's title in its row.
  RowTitle,
  /// Geist Mono 12 `text_muted`, line 17: a team server's address under its title.
  RowAddress,
  /// Geist Mono 13 `text_primary`, line 17: a server address or a code typed in its field (`yK0dX`).
  FieldMono,
}

impl StartupText {
  pub const ALL: [Self; 16] = [
    Self::TileIcon,
    Self::Crumb,
    Self::CrumbCurrent,
    Self::StepNumber,
    Self::StepLabel,
    Self::StepLabelCurrent,
    Self::Title,
    Self::Lead,
    Self::WayTitle,
    Self::Sub,
    Self::Aside,
    Self::Link,
    Self::Section,
    Self::RowTitle,
    Self::RowAddress,
    Self::FieldMono,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::TileIcon => "startup_tile_icon",
      Self::Crumb => "startup_crumb",
      Self::CrumbCurrent => "startup_crumb_current",
      Self::StepNumber => "startup_step_number",
      Self::StepLabel => "startup_step_label",
      Self::StepLabelCurrent => "startup_step_label_current",
      Self::Title => "startup_title",
      Self::Lead => "startup_lead",
      Self::WayTitle => "startup_way_title",
      Self::Sub => "startup_sub",
      Self::Aside => "startup_aside",
      Self::Link => "startup_link",
      Self::Section => "startup_section",
      Self::RowTitle => "startup_row_title",
      Self::RowAddress => "startup_row_address",
      Self::FieldMono => "startup_field_mono",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    let (primary, secondary, muted) = (palette.text_primary, palette.text_secondary, palette.text_muted);
    match self {
      Self::TileIcon => TextStyle {
        font_family: Arc::from(ICONS),
        ..style(20.0, FontWeight::Normal, 1.0, 0.0, secondary)
      },
      Self::Crumb => style(12.5, FontWeight::Normal, 16.0 / 12.5, 0.0, muted),
      Self::CrumbCurrent => style(12.5, FontWeight::Medium, 16.0 / 12.5, 0.0, primary),
      Self::StepNumber => style(10.5, FontWeight::Medium, 13.0 / 10.5, 0.0, primary),
      Self::StepLabel => style(12.0, FontWeight::Normal, 16.0 / 12.0, 0.0, muted),
      Self::StepLabelCurrent => style(12.0, FontWeight::Medium, 16.0 / 12.0, 0.0, primary),
      Self::Title => style(22.0, FontWeight::SemiBold, 28.0 / 22.0, -0.4, primary),
      Self::Lead => style(13.5, FontWeight::Normal, 21.0 / 13.5, 0.0, secondary),
      Self::WayTitle => style(15.0, FontWeight::SemiBold, 20.0 / 15.0, 0.0, primary),
      Self::Sub => style(12.0, FontWeight::Normal, 17.0 / 12.0, 0.0, muted),
      Self::Aside => style(12.5, FontWeight::Normal, 17.0 / 12.5, 0.0, muted),
      Self::Link => style(12.5, FontWeight::Medium, 17.0 / 12.5, 0.0, primary),
      Self::Section => style(11.0, FontWeight::Medium, 14.0 / 11.0, 0.4, muted),
      Self::RowTitle => style(13.5, FontWeight::Medium, 18.0 / 13.5, 0.0, primary),
      Self::RowAddress => mono(style(12.0, FontWeight::Normal, 17.0 / 12.0, 0.0, muted)),
      Self::FieldMono => mono(style(13.0, FontWeight::Normal, 17.0 / 13.0, 0.0, primary)),
    }
  }
}

/// Gaps and paddings of the startup screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupSpacing {
  /// 20: between the parts of a startup screen (`gap` of `g0Kj1`).
  Block,
  /// 40: inside the content panel around it (`Content · Startup` `RqhE7`).
  Panel,
  /// 28: above a step's page and between its parts (`Page` `qlOo2`).
  Page,
  /// 56: a step's page at its sides (`qlOo2`).
  PageSide,
  /// 10: between the steps of the step line and the lines between them (`F7crC`).
  Steps,
  /// 10: the account row's parts (`uNGRZ`), and a section's heading to its rows (`allZn`).
  Row,
}

impl StartupSpacing {
  pub const ALL: [Self; 6] = [
    Self::Block,
    Self::Panel,
    Self::Page,
    Self::PageSide,
    Self::Steps,
    Self::Row,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Block => "startup_block",
      Self::Panel => "startup_panel",
      Self::Page => "startup_page",
      Self::PageSide => "startup_page_side",
      Self::Steps => "startup_steps",
      Self::Row => "startup_row",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Block => 20.0,
      Self::Panel => 40.0,
      Self::Page => 28.0,
      Self::PageSide => 56.0,
      Self::Steps | Self::Row => 10.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  StartupText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in StartupSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
