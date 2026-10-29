//! Values of the Agents hub (`Agents · redesign` `ZsU3S` in the redesign file): its head and tabs (`Header` `LM3Mk`,
//! `Tabs` `Cc3JP`), the body's pane and side panel (`Body` `juZl6`, `Pane` `V7baZg`, `Side` `y8gUZ` in 01c), a
//! group's head over its rows (`Head` `dUbs7`, `Rows` `QpdBr`), a list row (`Builder` `fYNhS`), a form row (`Profile
//! name` `BzwDA`), a sub-page's crumb, head and draft bar (`Crumb` `g63gY`, `Page head` `O355d`, `Draft bar`
//! `W0sRO`) and a quiet note (`Note` `HVD56`).
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubText {
  /// 17/600 `text_primary`: an opened item's title in its page head (`WeZzq` in `O355d`).
  PageTitle,
  /// 12.5 `text_muted`: the crumb back to the tab's list (`OjREC` in `g63gY`).
  Crumb,
  /// 12/600 `text_secondary`: a group's or a section's title over its rows (`fdOM4` in `dUbs7`).
  GroupTitle,
  /// 12.5/500 `text_primary`: what a draft bar says first (`zLvEk` in `W0sRO`).
  BarStatus,
  /// 12.5 `text_primary`: a field's value (`YRez0` in `BzwDA`).
  Value,
  /// Geist Mono 12.5 `text_primary`, 1.5: an inline text area's lines, e.g. role instructions (`MC25H` in `CdDMo`).
  Area,
  /// 12 `text_muted`, 1.4: a row's hint, a group's count, a note (`zZmp4`, `B4SPLh`, `q3R6l`).
  Hint,
  /// 12 `text_secondary`: a row's status beside its dot (`PChrl` in `B6JYf`).
  Status,
  /// 12/500 `text_secondary`: what kind a row's item is before its meta line, e.g. a proposal's *Decision*
  /// (`Bnyni` in `f0kkti`).
  Kind,
  /// 13.5/500 `text_primary`, 1.45: what a memory proposal says (`AnUhX` in `f0kkti`).
  Statement,
  /// Geist Mono 13/500 `text_primary`: a row's label that is code, e.g. a variable (`wuoOU` in 02d `JXvc0`).
  Code,
}

impl HubText {
  pub const ALL: [Self; 11] = [
    Self::PageTitle,
    Self::Crumb,
    Self::GroupTitle,
    Self::BarStatus,
    Self::Value,
    Self::Area,
    Self::Hint,
    Self::Status,
    Self::Kind,
    Self::Statement,
    Self::Code,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::PageTitle => "hub_page_title",
      Self::Crumb => "hub_crumb",
      Self::GroupTitle => "hub_group_title",
      Self::BarStatus => "hub_bar_status",
      Self::Value => "hub_value",
      Self::Area => "hub_area",
      Self::Hint => "hub_hint",
      Self::Status => "hub_status",
      Self::Kind => "hub_kind",
      Self::Statement => "hub_statement",
      Self::Code => "hub_code",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::PageTitle => style(17.0, FontWeight::SemiBold, 1.25, 0.0, palette.text_primary),
      Self::Crumb => style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::GroupTitle => style(12.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_secondary),
      Self::BarStatus => style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
      Self::Value => style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::Area => mono(style(12.5, FontWeight::Normal, 1.5, 0.0, palette.text_primary)),
      Self::Hint => style(12.0, FontWeight::Normal, 1.4, 0.0, palette.text_muted),
      Self::Status => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::Kind => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.text_secondary),
      Self::Statement => style(13.5, FontWeight::Medium, 1.45, 0.0, palette.text_primary),
      Self::Code => mono(style(13.0, FontWeight::Medium, 1.3, 0.0, palette.text_primary)),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubSpacing {
  /// 28: the head's, tabs' and body's sides (`LM3Mk`, `Cc3JP`, `juZl6`).
  Side,
  /// 24: above the head's title (`LM3Mk`).
  HeadTop,
  /// 12: under the head, over the tabs (`LM3Mk`).
  HeadBottom,
  /// 20: above the body under the tabs (`juZl6`).
  BodyTop,
  /// 32: the pane to the side panel (`juZl6`).
  Beside,
  /// 28: between a list's groups (`V7baZg`).
  Groups,
  /// 24: between an opened item's sections (`rA12N`).
  Sections,
  /// 20: between the side panel's sections (`y8gUZ`).
  SideSections,
  /// 14: the crumb, the page head and the draft bar apart (`WSnxI`); a row's sides (`fYNhS`, `BzwDA`).
  Row,
  /// 10: over and under a list row (`fYNhS`).
  RowY,
  /// 11: over and under a form row (`BzwDA`).
  FieldY,
  /// 16: a form row's label to its control (`BzwDA`).
  FieldGap,
  /// 7: over and under a sub-head inside a group's rows, e.g. a role's (`ksfLm` in `V0UtB`).
  SubY,
  /// 28: a step's sub-head and its rules from the box's side (`mwisw`, `XWrFf` in `w31Kt`).
  Indent,
  /// 44: a block under a radio row from the box's side, past the radio and its gap (`A6i2K3`, `clbbm` in 07).
  Nested,
}

impl HubSpacing {
  pub const ALL: [Self; 15] = [
    Self::Side,
    Self::HeadTop,
    Self::HeadBottom,
    Self::BodyTop,
    Self::Beside,
    Self::Groups,
    Self::Sections,
    Self::SideSections,
    Self::Row,
    Self::RowY,
    Self::FieldY,
    Self::FieldGap,
    Self::SubY,
    Self::Indent,
    Self::Nested,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Side => "hub_side",
      Self::HeadTop => "hub_head_top",
      Self::HeadBottom => "hub_head_bottom",
      Self::BodyTop => "hub_body_top",
      Self::Beside => "hub_beside",
      Self::Groups => "hub_groups",
      Self::Sections => "hub_sections",
      Self::SideSections => "hub_side_sections",
      Self::Row => "hub_row",
      Self::RowY => "hub_row_y",
      Self::FieldY => "hub_field_y",
      Self::FieldGap => "hub_field_gap",
      Self::SubY => "hub_sub_y",
      Self::Indent => "hub_indent",
      Self::Nested => "hub_nested",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Side | Self::Groups | Self::Indent => 28.0,
      Self::HeadTop | Self::Sections => 24.0,
      Self::HeadBottom => 12.0,
      Self::BodyTop | Self::SideSections => 20.0,
      Self::Beside => 32.0,
      Self::Row => 14.0,
      Self::RowY => 10.0,
      Self::FieldY => 11.0,
      Self::FieldGap => 16.0,
      Self::SubY => 7.0,
      Self::Nested => 44.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in HubSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  HubText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
