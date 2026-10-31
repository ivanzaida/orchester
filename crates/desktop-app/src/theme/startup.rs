//! The design values of the startup screens (`Startup · Orchester is already open` `g0Kj1`, `Startup ·
//! Orchester can't open its data` `kMM56` and the variants board `ut8NI`) and of the start page and its steps
//! (`Startup · redesign` `qzzYw`: the first start `H9CZ1`, *Join a team* `B914L`, `drCQd`, the first download `YN6MO`)
//! that the UI kit's roles do not carry, registered as lurq `Extra` roles.

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
  /// Geist Mono 11.5 `text_muted`, line 15: the key of a task that arrived (`M0Kt9C`).
  ArrivalKey,
  /// 12.5 `text_primary`, line 17: the title of a task that arrived (`DBOw1`).
  ArrivalTitle,
  /// Geist Mono 12.5 `text_secondary`, line 17: the joined server's address over its projects (`fsVyJ`).
  ServerAddress,
  /// 13/500 `text_primary`, line 18: the project being linked (`gC6Dw`).
  ProjectName,
  /// Geist Mono 12 `text_secondary`, line 16: its address (`nrHKu`).
  ProjectAddress,
  /// 12 `text_secondary`, line 17: what the folder's check found, beside its mark (`tHuij`).
  CheckLine,
  /// 26/600 `text_primary`, 1.25, tracking -0.5: the first start's question (`HZTFS` in 01 `H9CZ1`).
  StartTitle,
  /// 16/600 `text_primary`, 1.3: a way in's title on its card (`YPoQx`).
  CardTitle,
  /// 12.5 `text_secondary`, 1.5: what a way in is, under its title (`nTfuL`).
  CardBody,
  /// 11.5 `text_muted`, 1.3: what a way in needs, over its actions (`zbB97`).
  CardMeta,
  /// 11.5 `text_muted`, 1.4: what stays on this computer, in the first start's footer (`Z9Zql0`).
  Footer,
}

impl StartupText {
  pub const ALL: [Self; 27] = [
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
    Self::ArrivalKey,
    Self::ArrivalTitle,
    Self::ServerAddress,
    Self::ProjectName,
    Self::ProjectAddress,
    Self::CheckLine,
    Self::StartTitle,
    Self::CardTitle,
    Self::CardBody,
    Self::CardMeta,
    Self::Footer,
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
      Self::ArrivalKey => "startup_arrival_key",
      Self::ArrivalTitle => "startup_arrival_title",
      Self::ServerAddress => "startup_server_address",
      Self::ProjectName => "startup_project_name",
      Self::ProjectAddress => "startup_project_address",
      Self::CheckLine => "startup_check_line",
      Self::StartTitle => "startup_start_title",
      Self::CardTitle => "startup_card_title",
      Self::CardBody => "startup_card_body",
      Self::CardMeta => "startup_card_meta",
      Self::Footer => "startup_footer",
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
      Self::ArrivalKey => mono(style(11.5, FontWeight::Normal, 15.0 / 11.5, 0.0, muted)),
      Self::ArrivalTitle => style(12.5, FontWeight::Normal, 17.0 / 12.5, 0.0, primary),
      Self::ServerAddress => mono(style(12.5, FontWeight::Normal, 17.0 / 12.5, 0.0, secondary)),
      Self::ProjectName => style(13.0, FontWeight::Medium, 18.0 / 13.0, 0.0, primary),
      Self::ProjectAddress => mono(style(12.0, FontWeight::Normal, 16.0 / 12.0, 0.0, secondary)),
      Self::CheckLine => style(12.0, FontWeight::Normal, 17.0 / 12.0, 0.0, secondary),
      Self::StartTitle => style(26.0, FontWeight::SemiBold, 1.25, -0.5, primary),
      Self::CardTitle => style(16.0, FontWeight::SemiBold, 1.3, 0.0, primary),
      Self::CardBody => style(12.5, FontWeight::Normal, 1.5, 0.0, secondary),
      Self::CardMeta => style(11.5, FontWeight::Normal, 1.3, 0.0, muted),
      Self::Footer => style(11.5, FontWeight::Normal, 1.4, 0.0, muted),
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
  /// 26: what an option holds, past its 16 radio and the 10 after it (`Body` `X2MDcb`).
  OptionIndent,
  /// 10: between the mark, the eyebrow, the question and the lead of the first start (`Head` `A03a5`).
  Head,
}

impl StartupSpacing {
  pub const ALL: [Self; 8] = [
    Self::Block,
    Self::Panel,
    Self::Page,
    Self::PageSide,
    Self::Steps,
    Self::Row,
    Self::OptionIndent,
    Self::Head,
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
      Self::OptionIndent => "startup_option_indent",
      Self::Head => "startup_head",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Block => 20.0,
      Self::Panel => 40.0,
      Self::Page => 28.0,
      Self::PageSide => 56.0,
      Self::Steps | Self::Row | Self::Head => 10.0,
      Self::OptionIndent => 26.0,
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
