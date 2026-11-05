//! Values of a service's page under Project settings › Services (`02 · Service editor` `TGRTt` in
//! `Agent testing · redesign` `eWydp`): the chips of its start-order chain (`Start order` `Dybbp`), the rows of the
//! side column's cards (`Sow5a`, `FJny2`, `o62dBM`) and the UI driver's cards (`Picker` `AHbLE`).
use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServicesText {
  /// Geist Mono 11 `text_muted`: a chain chip's place in the start (`Num` `UbGRP`).
  ChainNumber,
  /// Geist Mono 12 `text_secondary`: a chain chip's service; the one being edited in `text_primary` (`zd2y5`,
  /// `f8hsD`).
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
  /// (`UMC4g`, `xCTPs`).
  SideLine,
}

impl ServicesText {
  pub const ALL: [Self; 6] = [
    Self::ChainNumber,
    Self::ChainName,
    Self::OrderNumber,
    Self::OrderName,
    Self::DriverName,
    Self::SideLine,
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
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::ChainNumber => mono(style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::ChainName | Self::OrderName => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::OrderNumber => mono(style(10.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::DriverName => mono(style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_secondary)),
      Self::SideLine => mono(style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ServicesText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
