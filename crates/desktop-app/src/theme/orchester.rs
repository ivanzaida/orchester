//! The design values of Settings › Orchester and *Connect a server* (`Cu20b`, `DUWM5`, `SFaAp`, `Aowlk`, `zxe2P`,
//! `bIwte` in the Settings layer `uyO5v`), Project settings › Sharing's *Link to a project* (`bWD9q`, `N9NFio`) and the
//! status bar's server item (`X39PHh`) that the UI kit's and Settings › Accounts' roles do not carry, registered as
//! lurq `Extra` roles. The list and the details panel reuse Settings › Accounts' roles: the designer drew them from the
//! same masters (`Account group head` `I6Ebu`, `Account list row` `LW6D5`).

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// Text of Settings › Orchester.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrchesterText {
  /// 18/600 `text_primary`: *Sign in to Orchester Cloud* over the form (`ugJlw` in `DUWM5`).
  FormTitle,
  /// 22/600 `text_primary`, tracking -0.3: *Connect a server* under its breadcrumb (`zHmtd` in `zxe2P`).
  PageTitle,
  /// 13/600 `text_primary`: a step's title in *Connect a server* (`DwjDQ` in `MrVql`); a later step's in
  /// `text_muted`.
  StepTitle,
  /// 10.5/600: the number in a step's dot (`oCb5r`), `text_inverse` on the current one, `text_muted` on a later one.
  StepNumber,
  /// Geist Mono 24/500, tracking 3: a pairing code to read out (`H8RW-2NCE` in `Aowlk`).
  Code,
  /// Geist Mono 16/500 `text_primary`: a server's check code to compare (`UWPnK` in `IBnMu`, `oyvby` in `ZUuMT`).
  CheckCode,
  /// 13/500 `text_primary`, 1.45: who invited the person to what (`Mc4yA` in `Cu20b`).
  InviteTitle,
  /// Geist Mono 11.5 `text_secondary`: a project's address in an invite or a project row (`kPNV8` in `Cu20b`).
  Address,
  /// 12.5/600 `text_primary`: a card's title, *What linking sends* (`CxQE4` in `bWD9q`).
  CardTitle,
}

impl OrchesterText {
  pub const ALL: [Self; 9] = [
    Self::FormTitle,
    Self::PageTitle,
    Self::StepTitle,
    Self::StepNumber,
    Self::Code,
    Self::CheckCode,
    Self::InviteTitle,
    Self::Address,
    Self::CardTitle,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::FormTitle => "orchester_form_title",
      Self::PageTitle => "orchester_page_title",
      Self::StepTitle => "orchester_step_title",
      Self::StepNumber => "orchester_step_number",
      Self::Code => "orchester_code",
      Self::CheckCode => "orchester_check_code",
      Self::InviteTitle => "orchester_invite_title",
      Self::Address => "orchester_address",
      Self::CardTitle => "orchester_card_title",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::FormTitle => style(18.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::PageTitle => style(22.0, FontWeight::SemiBold, 1.3, -0.3, palette.text_primary),
      Self::StepTitle => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::StepNumber => style(10.5, FontWeight::SemiBold, 1.0, 0.0, palette.text_inverse),
      Self::Code => mono(style(24.0, FontWeight::Medium, 1.3, 3.0, palette.text_primary)),
      Self::CheckCode => mono(style(16.0, FontWeight::Medium, 1.3, 0.0, palette.text_primary)),
      Self::InviteTitle => style(13.0, FontWeight::Medium, 1.45, 0.0, palette.text_primary),
      Self::Address => mono(style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::CardTitle => style(12.5, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
    }
  }
}

/// Gaps and paddings of Settings › Orchester and *Connect a server*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrchesterSpacing {
  /// 40: the signed-out panel's left side, where the form starts (`qnmP2` in `DUWM5`).
  FormSide,
  /// 22: the signed-out panel's top (`qnmP2`); a step's room under its content (`oZvKI` in `zxe2P`).
  FormTop,
  /// 14: between the form's parts (`Ay4fx`); between a step's rail and content (`AhrFW`).
  FormGap,
  /// 96: *Connect a server*'s sides (`P2GyHo`, `Pkcy0` in `zxe2P`).
  PageSide,
  /// 18: inside a code's box at its sides (`ofYYa`).
  CodeSide,
}

impl OrchesterSpacing {
  pub const ALL: [Self; 5] = [
    Self::FormSide,
    Self::FormTop,
    Self::FormGap,
    Self::PageSide,
    Self::CodeSide,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::FormSide => "orchester_form_side",
      Self::FormTop => "orchester_form_top",
      Self::FormGap => "orchester_form_gap",
      Self::PageSide => "orchester_page_side",
      Self::CodeSide => "orchester_code_side",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::FormSide => 40.0,
      Self::FormTop => 22.0,
      Self::FormGap => 14.0,
      Self::PageSide => 96.0,
      Self::CodeSide => 18.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  OrchesterText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in OrchesterSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
