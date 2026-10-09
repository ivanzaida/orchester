//! The shell's design values that no shared role carries yet, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::{fonts::MONO, typography::style};

/// Text of the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellText {
  /// 12/600 `text_secondary`: the project's initial in the switcher's tile (`Letter` `osv95`).
  SwitcherInitial,
  /// Geist Mono 10.5 `text_muted`: the project's folder under its name in the switcher (`Path` `RJaZX`).
  SwitcherPath,
}

impl ShellText {
  pub const ALL: [Self; 2] = [Self::SwitcherInitial, Self::SwitcherPath];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::SwitcherInitial => "shell_switcher_initial",
      Self::SwitcherPath => "shell_switcher_path",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::SwitcherInitial => style(12.0, FontWeight::SemiBold, 1.35, 0.0, palette.text_secondary),
      Self::SwitcherPath => TextStyle {
        font_family: Arc::from(MONO),
        ..style(10.5, FontWeight::Normal, 1.35, 0.0, palette.text_muted)
      },
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ShellText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
