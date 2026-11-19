//! Values of choosing how a subscription takes its target's work, on the board `Orchestrator autonomy · frames to
//! approve` `uYCy4` (OA-10): the /watch picker's *Follow* / *Lead to the end* (`Mode` `GA2bV` in `L03` `xE361`), the
//! mode control and the ceilings row of the `/watching` answer (`ccoKs`, `q5tvtE` in `L02` `HpcQJ`) and the ceilings'
//! form (`Ea9jV` in `Parts · Ceilings` `H6wvm`). What they share with the rest of the app are the kit's roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::{
  palette::text_secondary,
  typography::{mono, style},
};

/// The ceilings' form and the mode's menu (`Ceilings form` `Ea9jV`, `Mode menu` `ZHAjU`).
pub const CEILINGS_FORM_WIDTH: f32 = 440.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeadingText {
  /// 11.5 `text_secondary`, 1.3: a way the picker's control does not have chosen (`qkjPe` in `GA2bV`), and the way
  /// on a subscription's row (`omouE` in `ccoKs`).
  Way,
  /// 11.5 `text_muted`, 1.45: what the chosen way means, under the picker's control (`R0FlD` in `GA2bV`).
  Meaning,
  /// Geist Mono 11 `text_secondary`, 1.3: a ceiling's value on its row (`xnfVf` in `q5tvtE`).
  Ceiling,
  /// 12 `text_muted`, 1.3: *by*, *%* and *tokens* beside a ceiling's field (`lySl8`, `hCLOg` in `Ea9jV`).
  Unit,
}

impl LeadingText {
  pub const ALL: [Self; 4] = [Self::Way, Self::Meaning, Self::Ceiling, Self::Unit];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Way => "leading_way",
      Self::Meaning => "leading_meaning",
      Self::Ceiling => "leading_ceiling",
      Self::Unit => "leading_unit",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Way => style(11.5, FontWeight::Normal, 1.3, 0.0, text_secondary(palette)),
      Self::Meaning => style(11.5, FontWeight::Normal, 1.45, 0.0, palette.muted_foreground),
      Self::Ceiling => mono(style(11.0, FontWeight::Normal, 1.3, 0.0, text_secondary(palette))),
      Self::Unit => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.muted_foreground),
    }
  }
}

/// Gaps and paddings of the mode's controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeadingSpacing {
  /// 9: the sides of a way in the picker's control (`HURW1`, `dKaEd` in `jK9Or`).
  WaySide,
  /// 7: the sides of the mode control on a subscription's row (`ccoKs`).
  ControlSide,
}

impl LeadingSpacing {
  pub const ALL: [Self; 2] = [Self::WaySide, Self::ControlSide];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::WaySide => "leading_way_side",
      Self::ControlSide => "leading_control_side",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::WaySide => 9.0,
      Self::ControlSide => 7.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  LeadingText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in LeadingSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
