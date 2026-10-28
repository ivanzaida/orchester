//! The shell's design values that no shared role carries yet, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// Text of the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellText {
  /// 12/600 `text_secondary`: the project's initial in the switcher's tile (`Letter` `osv95`).
  SwitcherInitial,
  /// Geist Mono 10.5 `text_muted`: the project's folder under its name in the switcher (`Path` `RJaZX`).
  SwitcherPath,
  /// 10.5/500 `text_muted`, tracking 0.6: a section of the project switcher's menu (`RECENT`, `K5Y25I`).
  MenuSection,
  /// 12.5/500 `text_primary`, 1.4: why a chosen folder does not open, as the switcher says it (`jyvLs`).
  FolderTitle,
  /// 11.5 `text_secondary`, 1.45: what the switcher explains under it (`JHqGF`).
  FolderBody,
  /// 12.5 `text_primary`: a result of *Jump to* (`Main` `KeDEy` in `aDYAX`).
  JumpResult,
  /// 11.5 `text_muted`: how many results *Jump to* shows (`Count` `qiz3y` in `aDYAX`).
  JumpCount,
}

impl ShellText {
  pub const ALL: [Self; 7] = [
    Self::SwitcherInitial,
    Self::SwitcherPath,
    Self::MenuSection,
    Self::FolderTitle,
    Self::FolderBody,
    Self::JumpResult,
    Self::JumpCount,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::SwitcherInitial => "shell_switcher_initial",
      Self::SwitcherPath => "shell_switcher_path",
      Self::MenuSection => "shell_menu_section",
      Self::FolderTitle => "shell_folder_title",
      Self::FolderBody => "shell_folder_body",
      Self::JumpResult => "shell_jump_result",
      Self::JumpCount => "shell_jump_count",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::SwitcherInitial => style(12.0, FontWeight::SemiBold, 1.35, 0.0, palette.text_secondary),
      Self::SwitcherPath => mono(style(10.5, FontWeight::Normal, 1.35, 0.0, palette.text_muted)),
      Self::MenuSection => style(10.5, FontWeight::Medium, 1.35, 0.6, palette.text_muted),
      Self::FolderTitle => style(12.5, FontWeight::Medium, 1.4, 0.0, palette.text_primary),
      Self::FolderBody => style(11.5, FontWeight::Normal, 1.45, 0.0, palette.text_secondary),
      Self::JumpResult => style(12.5, FontWeight::Normal, 1.35, 0.0, palette.text_primary),
      Self::JumpCount => style(11.5, FontWeight::Normal, 1.35, 0.0, palette.text_muted),
    }
  }
}

/// Gaps and paddings of the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellSpacing {
  /// 22: a refused folder's actions, indented past its 14 icon and the 8 after it (`Actions` `vg7Gl`).
  FolderActions,
}

impl ShellSpacing {
  pub const ALL: [Self; 1] = [Self::FolderActions];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::FolderActions => "shell_folder_actions",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::FolderActions => 22.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ShellText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in ShellSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
