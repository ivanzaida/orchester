//! The design values of Settings › Accounts (layer `hgOl2`; `Account group head` `I6Ebu`, `Account list row` `LW6D5`,
//! `Plan window row` `B9myCE`, the details panel of `t098v`) that the UI kit's roles do not carry, registered as lurq
//! `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// Text of Settings › Accounts. The rest reuses the kit's roles: a row's name `OptionTitle`, its folder `MonoNote`,
/// a count `Caption`, a note `Note`, a paragraph `Prose`, a plan window's name `Description`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountsText {
  /// 11/600 `text_secondary`: a CLI's name over its accounts (`R7vGW4` in `I6Ebu`).
  GroupName,
  /// 11.5/500: an account's state beside its icon, in the state's color (`QIWCp` in `LW6D5`).
  State,
  /// 11.5 `text_secondary`: who is signed in and the plan, or what to do (`u8Bdu1`).
  Identity,
  /// 15/600 `text_primary`: the shown account's name (`yyjVB` in `t098v`).
  Title,
}

impl AccountsText {
  pub const ALL: [Self; 4] = [Self::GroupName, Self::State, Self::Identity, Self::Title];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::GroupName => "accounts_group_name",
      Self::State => "accounts_state",
      Self::Identity => "accounts_identity",
      Self::Title => "accounts_title",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::GroupName => style(11.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_secondary),
      Self::State => style(11.5, FontWeight::Medium, 1.3, 0.0, palette.text_secondary),
      Self::Identity => style(11.5, FontWeight::Normal, 1.4, 0.0, palette.text_secondary),
      Self::Title => style(15.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
    }
  }
}

/// Gaps and paddings of Settings › Accounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountsSpacing {
  /// 14: a list row's and a group head's sides (`LW6D5`, `I6Ebu`).
  Edge,
  /// 12: a list row's top and bottom, its glyph to its text, a plan window's parts (`LW6D5`, `B9myCE`).
  Cell,
  /// 3: between a row's lines (`qcuSX`).
  Lines,
  /// 18: the details panel's header sides, its tabs' sides and its body (`jCbMF`, `y2N9a`, `pwIrF`).
  Panel,
  /// 20: between the details' parts (`pwIrF`).
  Parts,
  /// 24: between the list and the details (`xjznQ`).
  Columns,
  /// 18: past a note's 12 icon and its 6 gap, where a choice under the note starts (`Also move …` in `HzJSR`).
  NoteIndent,
}

impl AccountsSpacing {
  pub const ALL: [Self; 7] = [
    Self::Edge,
    Self::Cell,
    Self::Lines,
    Self::Panel,
    Self::Parts,
    Self::Columns,
    Self::NoteIndent,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Edge => "accounts_edge",
      Self::Cell => "accounts_cell",
      Self::Lines => "accounts_lines",
      Self::Panel => "accounts_panel",
      Self::Parts => "accounts_parts",
      Self::Columns => "accounts_columns",
      Self::NoteIndent => "accounts_note_indent",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Edge => 14.0,
      Self::Cell => 12.0,
      Self::Lines => 3.0,
      Self::Panel | Self::NoteIndent => 18.0,
      Self::Parts => 20.0,
      Self::Columns => 24.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  AccountsText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in AccountsSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
