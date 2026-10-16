//! The design values of the sign-in gate (layer *Sign-in gate* `ASl21`: `Lpznn`, `Rmtl1` and the states beside them)
//! that the UI kit's and Settings › Orchester's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// Text of the sign-in gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateText {
  /// Geist Mono 11 `text_muted`, tracking 0.6: *WELCOME TO ORCHESTER* over the title (`VO3Sd` in `Lpznn`).
  Eyebrow,
  /// 26/600 `text_primary`, tracking -0.5: *Sign in to start* (`oLIKj`).
  Title,
  /// 13.5 `text_secondary`, 1.55: what the gate is for, under the title (`p0M4CT`).
  Lead,
  /// 11.5 `text_muted`, 1.45: what the two pages in the browser are for, at the card's foot (`Browser note` `hiNvS`).
  Note,
}

impl GateText {
  pub const ALL: [Self; 4] = [Self::Eyebrow, Self::Title, Self::Lead, Self::Note];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Eyebrow => "gate_eyebrow",
      Self::Title => "gate_title",
      Self::Lead => "gate_lead",
      Self::Note => "gate_note",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Eyebrow => mono(style(11.0, FontWeight::Normal, 1.3, 0.6, palette.text_muted)),
      Self::Title => style(26.0, FontWeight::SemiBold, 1.3, -0.5, palette.text_primary),
      Self::Lead => style(13.5, FontWeight::Normal, 1.55, 0.0, palette.text_secondary),
      Self::Note => style(11.5, FontWeight::Normal, 1.45, 0.0, palette.text_muted),
    }
  }
}

/// Gaps and paddings of the sign-in gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateSpacing {
  /// 20: between the gate column's parts: the head, the notice, the form and *Advanced* (`K4UfUq`).
  Column,
  /// 56: the gate's room at its sides (`Main` `u1AnQ`).
  Side,
}

impl GateSpacing {
  pub const ALL: [Self; 2] = [Self::Column, Self::Side];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Column => "gate_column",
      Self::Side => "gate_side",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Column => 20.0,
      Self::Side => 56.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  GateText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in GateSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
