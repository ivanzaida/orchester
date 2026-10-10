//! The design values of a run's changes on the task page (`Task · Changes · dark` `DYun5`, its parts in
//! `Task page · components` `n70u8y`) that the UI kit's roles do not carry, registered as lurq `Extra`
//! roles. Code itself is `TypographyStyle::Mono` (Geist Mono 12, `text_primary`), line numbers
//! `ExtraTypography::MonoNote` and a hunk's heading `ExtraTypography::MonoKey`; tokens take the `code_*`
//! colors.

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};
use orchester_workspace::highlight::TokenKind;

use super::{
  ExtraColor,
  typography::{mono, style},
};

/// Text of the Changes tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeText {
  /// Geist Mono 11/600 `text_secondary`: the letter of a file's change, `M` or `A` (`F1t7T` in `dAwTi`,
  /// `t1Jjzb` in `rIRdw`).
  ChangeType,
  /// Geist Mono 12.5/500 `text_primary`: the path over a file's diff (`ET1Yg` in `rIRdw`).
  DiffPath,
  /// 10.5/500 `text_muted`, tracking 0.5: a group's heading in the file list, `CHECKPOINTED` (`DVtCU`).
  GroupHeading,
  /// Geist Mono 10.5 `text_muted`: how many files a group lists (`XwfXk`).
  GroupCount,
}

impl CodeText {
  pub const ALL: [Self; 4] = [Self::ChangeType, Self::DiffPath, Self::GroupHeading, Self::GroupCount];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::ChangeType => "code_change_type",
      Self::DiffPath => "code_diff_path",
      Self::GroupHeading => "code_group_heading",
      Self::GroupCount => "code_group_count",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::ChangeType => mono(style(11.0, FontWeight::SemiBold, 1.4, 0.0, palette.text_secondary)),
      Self::DiffPath => mono(style(12.5, FontWeight::Medium, 1.4, 0.0, palette.text_primary)),
      Self::GroupHeading => style(10.5, FontWeight::Medium, 1.35, 0.5, palette.text_muted),
      Self::GroupCount => mono(style(10.5, FontWeight::Normal, 1.35, 0.0, palette.text_muted)),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  CodeText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

/// The color of a highlighted token: the design colors keywords, strings and numbers (constants), and
/// comments in `text_muted`; functions and names read as text (`Code holds token texts` in `n70u8y`).
pub fn token_color(kind: TokenKind) -> PaletteColor {
  match kind {
    TokenKind::Keyword => ExtraColor::CodeKeyword.role(),
    TokenKind::String => ExtraColor::CodeString.role(),
    TokenKind::Constant => ExtraColor::CodeNumber.role(),
    TokenKind::Comment => PaletteColor::TextMuted,
    TokenKind::Plain | TokenKind::Function | TokenKind::Variable => PaletteColor::TextPrimary,
  }
}
