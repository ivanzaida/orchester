//! Values of the service cards in a run's Conversation (`Agent testing · redesign` `eWydp`): the service crashed card
//! (`Card · cloud-api crashed` `tz1d8` in `FwHrv`), the approve services card (`Card · approve services` `TienU` in
//! `hvQDS`) and the states of `10 · States` `oXVuM` (10a `SGntT`, 10c `DPLN1`, 10d `qsBGf`).
use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceCardsText {
  /// 12.5 `text_secondary`, 1.5: what happened and what Orchester did (`JNqgZ`, `jb1yu`, `mYl7Q`, `E2VhX`, `Li0D4`).
  Body,
  /// Geist Mono 12 `text_secondary`: a service's program and arguments in the approve services card (`GZxEp`).
  Command,
  /// 12 `text_muted`: whether a service's start is approved, at a program's end (`jBOKa`; `warning` for a change,
  /// `info` for a first start), the slots line (`XR7xh`) and the macOS line of 10a (`s3ehm`).
  Detail,
}

impl ServiceCardsText {
  pub const ALL: [Self; 3] = [Self::Body, Self::Command, Self::Detail];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Body => "service_cards_body",
      Self::Command => "service_cards_command",
      Self::Detail => "service_cards_detail",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Body => style(12.5, FontWeight::Normal, 1.5, 0.0, palette.text_secondary),
      Self::Command => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::Detail => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ServiceCardsText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
