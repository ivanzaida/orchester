//! Values of a run's pull request CI (ORC-335; the Summary's card `sSYu8` in 10 `ysTLm`, `J0SbA` in 10b `mxt1S`,
//! `DLnt0` in 10c `YBx9T`, `p7oVdN` in 10d `uA3vm`; the Conversation's card in 11 `MK7l7` and the parts in 11d `kLZUJ`)
//! that the UI kit's roles do not carry.

use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CiText {
  /// Geist Mono 11.5 `text_secondary`, 1.6: a failing check's log tail on `code_bg` (its last line in `text_primary`).
  LogTail,
}

impl CiText {
  pub const ALL: [Self; 1] = [Self::LogTail];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::LogTail => "ci_log_tail",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::LogTail => mono(style(11.5, FontWeight::Normal, 1.6, 0.0, palette.text_secondary)),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  CiText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
