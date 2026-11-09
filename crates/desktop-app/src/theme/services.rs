//! Values of the pages under Project settings › Services (`Agent testing · redesign` `eWydp`): a service's page
//! (`02 · Service editor` `TGRTt`) with the chips of its start-order chain (`Start order` `Dybbp`), the rows of the
//! side column's cards (`Sow5a`, `FJny2`, `o62dBM`) and the UI driver's cards (`Picker` `AHbLE`); a stack's page
//! (`03 · Stack editor` `iNxTM`) with its services' step marks (`Step` `a0o7Uf`), the slots' bar (`Bar` `q9HZT`) and
//! the tree of needs (`Dependencies` `z1aSH`).
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServicesText {
  /// Geist Mono 11 `text_muted`: a chain chip's place in the start (`Num` `UbGRP`).
  ChainNumber,
  /// Geist Mono 12 `text_secondary`: a chain chip's service; the one being edited in `text_primary` (`zd2y5`,
  /// `f8hsD`); a line of the tree of needs (`iud7A` in `z1aSH`).
  ChainName,
  /// Geist Mono 10 `text_secondary`: a start order row's place in its round mark (`tSqix`); the one being edited on
  /// `accent` in `on_accent` (`EJrkR`).
  OrderNumber,
  /// Geist Mono 12 `text_secondary`: a start order row's service; the one being edited in `text_primary` (`q3WMp`,
  /// `E3iBP`).
  OrderName,
  /// Geist Mono 12.5/500 `text_secondary`: a UI driver card's name; the chosen one's in `text_primary` (`Uk9us`,
  /// `wVFNF`).
  DriverName,
  /// Geist Mono 11.5 `text_secondary`: a placeholder in *Values in a run* and a tool's call in *Agents use it with*
  /// (`UMC4g`, `xCTPs`, `sSP1V` in `uxYcD`).
  SideLine,
  /// Geist Mono 10.5 `text_secondary`: the step a stack's service starts on, in its round mark (`L5UCWh` in `a0o7Uf`).
  StepNumber,
}

impl ServicesText {
  pub const ALL: [Self; 7] = [
    Self::ChainNumber,
    Self::ChainName,
    Self::OrderNumber,
    Self::OrderName,
    Self::DriverName,
    Self::SideLine,
    Self::StepNumber,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::ChainNumber => "services_chain_number",
      Self::ChainName => "services_chain_name",
      Self::OrderNumber => "services_order_number",
      Self::OrderName => "services_order_name",
      Self::DriverName => "services_driver_name",
      Self::SideLine => "services_side_line",
      Self::StepNumber => "services_step_number",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::ChainNumber => mono(style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::ChainName | Self::OrderName => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::OrderNumber => mono(style(10.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::DriverName => mono(style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_secondary)),
      Self::SideLine => mono(style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::StepNumber => mono(style(10.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ServicesText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServicesSpacing {
  /// 18: how much further in each level of the tree of needs starts (`cloud-api` `ePZt6`, `cloud-web` `E9Ghu`).
  TreeIndent,
  /// 4: between the slots of the slots' bar (`Slots` `QXz4w`).
  SlotGap,
}

impl ServicesSpacing {
  pub const ALL: [Self; 2] = [Self::TreeIndent, Self::SlotGap];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::TreeIndent => "services_tree_indent",
      Self::SlotGap => "services_slot_gap",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::TreeIndent => 18.0,
      Self::SlotGap => 4.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in ServicesSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
