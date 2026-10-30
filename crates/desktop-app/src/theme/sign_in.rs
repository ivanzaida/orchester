//! The design values of the Orchester Cloud sign-in (board *Signed out · redesign* `uBND2`: `j7SbPk`, `wpD5E`, `eWsUb`)
//! that the UI kit's and Settings › Orchester's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// Text of the sign-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignInText {
  /// 13/600 `text_primary`: *Not signed in*, *Sign in again* and *Enter your code* over the card (`j7SbPk`, `eWsUb`).
  Title,
  /// 20 `text_primary`: a digit in its box (`eWsUb`).
  Digit,
  /// Geist Mono 13 `text_primary`, tracking 2: a recovery code in its field.
  Code,
}

impl SignInText {
  pub const ALL: [Self; 3] = [Self::Title, Self::Digit, Self::Code];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Title => "sign_in_title",
      Self::Digit => "sign_in_digit",
      Self::Code => "sign_in_code",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Title => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Digit => style(20.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::Code => mono(style(13.0, FontWeight::Normal, 1.3, 2.0, palette.text_primary)),
    }
  }
}

/// Gaps of the sign-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignInSpacing {
  /// 7: between the boxes of a code (`eWsUb`).
  Digits,
}

impl SignInSpacing {
  pub const ALL: [Self; 1] = [Self::Digits];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Digits => "sign_in_digits",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Digits => 7.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  SignInText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in SignInSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
