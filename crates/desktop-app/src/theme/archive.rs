//! Values of the archive confirmation (`A23 · Archive · used by · confirmation` `Pnpbe`, local `L11` `dS97b`) and of
//! the stopped run's card on the task page (`A22` `pUu78`).
use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// The confirmation's width (`Dialog` `MdjVb`).
pub const ARCHIVE_DIALOG_WIDTH: f32 = 480.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveText {
  /// 14/600 `text_primary`, 1.35: the question (`Title` `O4gVO1`).
  Title,
  /// 12.5 `text_secondary`, 1.5: what archiving does (`Body` `pkoCV`).
  Body,
  /// 11.5/600 `text_muted`, 1.45: a list's heading, *USED BY · 3* (`Heading` `zftF0`).
  Heading,
  /// 12.5 `text_primary`, 1.45: what a list row names (`Name` `D5hiY`).
  Name,
  /// 12.5 `text_secondary`, 1.45: whose run it is (`Detail` `jz3an`).
  Holder,
  /// Geist Mono 12 `text_muted`, 1.45: the version at a row's end (`Detail` `opOyX`).
  Version,
  /// 12 `text_muted`, 1.5: what stays not ready (`Note` `u3ZuS`).
  Note,
}

impl ArchiveText {
  pub const ALL: [Self; 7] = [
    Self::Title,
    Self::Body,
    Self::Heading,
    Self::Name,
    Self::Holder,
    Self::Version,
    Self::Note,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Title => "archive_title",
      Self::Body => "archive_body",
      Self::Heading => "archive_heading",
      Self::Name => "archive_name",
      Self::Holder => "archive_holder",
      Self::Version => "archive_version",
      Self::Note => "archive_note",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Title => style(14.0, FontWeight::SemiBold, 1.35, 0.0, palette.text_primary),
      Self::Body => style(12.5, FontWeight::Normal, 1.5, 0.0, palette.text_secondary),
      Self::Heading => style(11.5, FontWeight::SemiBold, 1.45, 0.0, palette.text_muted),
      Self::Name => style(12.5, FontWeight::Normal, 1.45, 0.0, palette.text_primary),
      Self::Holder => style(12.5, FontWeight::Normal, 1.45, 0.0, palette.text_secondary),
      Self::Version => mono(style(12.0, FontWeight::Normal, 1.45, 0.0, palette.text_muted)),
      Self::Note => style(12.0, FontWeight::Normal, 1.5, 0.0, palette.text_muted),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ArchiveText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
