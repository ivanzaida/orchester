//! Values of the Tests card on a task's Summary (`07 · Summary · Tests` `qN2FK` in `Agent testing · redesign`
//! `eWydp`): a layer's name, a test's name and file, the lines of a failed test's message, and the card's own headings.
use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestsText {
  /// Geist Mono 12.5/500 `text_primary`: a layer's name, `lint` (`yHyXs`), and a failed test's (`NC2ol`).
  Name,
  /// Geist Mono 12.5 `text_primary`: a flaky test's name (`desMD`).
  FlakyName,
  /// Geist Mono 12 `text_muted`: a test's file:line (`fV869`) and a layer's time, `38 s` (`K9toyB`).
  Detail,
  /// Geist Mono 11.5 `text_secondary`: a line of a failed test's message (`VolEf`); its first in `danger` (`PREAm`).
  Message,
  /// 12/600 `text_secondary`: *Failed*, *Flaky* and *Artifacts* over their rows (`XjDnk`, `K15U8W`, `p2qZx`).
  Heading,
}

impl TestsText {
  pub const ALL: [Self; 5] = [Self::Name, Self::FlakyName, Self::Detail, Self::Message, Self::Heading];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Name => "tests_name",
      Self::FlakyName => "tests_flaky_name",
      Self::Detail => "tests_detail",
      Self::Message => "tests_message",
      Self::Heading => "tests_heading",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Name => mono(style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary)),
      Self::FlakyName => mono(style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_primary)),
      Self::Detail => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::Message => mono(style(11.5, FontWeight::Normal, 1.4, 0.0, palette.text_secondary)),
      Self::Heading => style(12.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_secondary),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  TestsText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
