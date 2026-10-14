//! The design values of Local integration (section `zMOBO`: `YxF6B`, `kexy4`, `UgHeC`, `s4gj9`) that the UI kit's
//! roles do not carry, registered as lurq `Extra` roles. The rest reuses the kit's: the phase trail
//! `ExtraTypography::NoteStrong`, a list's title `Body`, a hunk's range `ExtraTypography::MonoKey`, code `Mono`, a
//! change's letter `CodeText::ChangeType`.

use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// The design's line box for 11.5 text: 17 (`Xf3kg`, `aT5aE`).
const SMALL_LINE: f32 = 17.0 / 11.5;

/// Text of Local integration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalText {
  /// 14 `text_primary`, 21 high: a notice's and an inspector card's heading (`AqWpx` in `t6PHFE`, `JTRPW`).
  Heading,
  /// 11.5 `text_secondary`: what a notice or card explains, a field's label, a recovery value (`Xf3kg`, `jPNSC`).
  Explanation,
  /// 11.5 `text_muted`: a snapshot's caption, a group's label, a boundary or guard note (`aT5aE`, `J0xZj`, `c1zep`).
  Note,
  /// Geist Mono 12 `text_primary`, 16 high: a changed path, wrapping where its row ends (`hFTW1` in `dAwTi`).
  Path,
}

impl LocalText {
  pub const ALL: [Self; 4] = [Self::Heading, Self::Explanation, Self::Note, Self::Path];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Heading => "local_heading",
      Self::Explanation => "local_explanation",
      Self::Note => "local_note",
      Self::Path => "local_path",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Heading => style(14.0, FontWeight::Normal, 1.5, 0.0, palette.text_primary),
      Self::Explanation => style(11.5, FontWeight::Normal, SMALL_LINE, 0.0, palette.text_secondary),
      Self::Note => style(11.5, FontWeight::Normal, SMALL_LINE, 0.0, palette.text_muted),
      Self::Path => mono(style(12.0, FontWeight::Normal, 16.0 / 12.0, 0.0, palette.text_primary)),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  LocalText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
