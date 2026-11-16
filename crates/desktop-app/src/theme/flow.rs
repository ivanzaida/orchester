//! Type roles of the steps drawn as the flow they run in (`Steps` `uSoll` in `Pipelines · v2` P1 `s7LMyQ`, P8 `vujkk`).

use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// A card whose place is faded while its drag is under way (`Card · 05 Review` `s1SIUj` in P3 `K4B56`).
pub const DRAGGED_OPACITY: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowText {
  /// Geist Mono 10.5 `text_muted`, spaced 0.6, 1.2: *ON TASK START*, *RUN IN PARALLEL* and *MERGED INTO …* (`r7IIY5`,
  /// `UVGsy`, `W5F8Hk`).
  Label,
  /// 12 `text_muted`, 1.2: what a start, a parallel group, a gate and an end say after their label (`RexHV`, `ZyWJi`,
  /// `g2WVCK`, `CnuAo`); *Choose profile* on an amber card in `warning` (`sBuad` in P8).
  Hint,
  /// 12/500 `warning`, 1.2: *Waits for your approval* (`OptVp`).
  Gate,
  /// Geist Mono 10.5 `text_secondary`, 1.2: a step's number in its node (`aRdE6`).
  Number,
  /// Geist Mono 11.5 `text_muted`, 1.2: a number in front of a card's name inside a parallel group (`XLjJT`).
  CardNumber,
  /// 12.5/500 `text_primary`, 1.3: a card's name (`Jg3Pb`).
  Name,
  /// 12 `text_secondary`, 1.3: what a card runs, and a failure route in `text_muted` (`I9Z0s2`, `DDlSK`); an amber
  /// card's problem in `warning` (`jz6zu` in P8).
  Line,
  /// 12/500 `text_primary`, 1.2: what a dragged card's drop line says first, *05 Review goes here* (`XHcfs` in `Drop
  /// here` `y0SHa`, P3).
  Landing,
}

impl FlowText {
  pub const ALL: [Self; 8] = [
    Self::Label,
    Self::Hint,
    Self::Gate,
    Self::Number,
    Self::CardNumber,
    Self::Name,
    Self::Line,
    Self::Landing,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Label => "flow_label",
      Self::Hint => "flow_hint",
      Self::Gate => "flow_gate",
      Self::Number => "flow_number",
      Self::CardNumber => "flow_card_number",
      Self::Name => "flow_name",
      Self::Line => "flow_line",
      Self::Landing => "flow_landing",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Label => mono(style(10.5, FontWeight::Normal, 1.2, 0.6, palette.text_muted)),
      Self::Hint => style(12.0, FontWeight::Normal, 1.2, 0.0, palette.text_muted),
      Self::Gate => style(12.0, FontWeight::Medium, 1.2, 0.0, palette.warning),
      Self::Number => mono(style(10.5, FontWeight::Normal, 1.2, 0.0, palette.text_secondary)),
      Self::CardNumber => mono(style(11.5, FontWeight::Normal, 1.2, 0.0, palette.text_muted)),
      Self::Name => style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
      Self::Line => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::Landing => style(12.0, FontWeight::Medium, 1.2, 0.0, palette.text_primary),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  FlowText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
