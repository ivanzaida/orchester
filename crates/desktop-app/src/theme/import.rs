//! Values of Import resources (`Dzzks`) that no shared role has: the stepper's numbers, the destination's tile, the
//! mono eyebrows over its groups and a secret's name in a mapping row, and the room of its steps' bodies.
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportText {
  /// 10.5/600 `text_inverse`: the current step's number (`k5ymI` in `Import step/current` `oFevv`).
  StepCurrent,
  /// 10.5 `text_muted`: a step still to come (`rKOXf` in `Import step/next` `MhTGv`).
  StepNext,
  /// 10/600 `text_secondary`: the destination's initial (`gtFlr`).
  Initial,
  /// Geist Mono 10.5 `text_muted`, tracking 0.6: a group's eyebrow (`KRlEH`, `JsK2W`, `z6HewZ`).
  Eyebrow,
  /// Geist Mono 12.5/500 `text_primary`: a secret's name in a mapping row (`oOtgw` in `ItSwZ`).
  SecretName,
}

impl ImportText {
  pub const ALL: [Self; 5] = [
    Self::StepCurrent,
    Self::StepNext,
    Self::Initial,
    Self::Eyebrow,
    Self::SecretName,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::StepCurrent => "import_step_current",
      Self::StepNext => "import_step_next",
      Self::Initial => "import_initial",
      Self::Eyebrow => "import_eyebrow",
      Self::SecretName => "import_secret_name",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::StepCurrent => style(10.5, FontWeight::SemiBold, 1.0, 0.0, palette.text_inverse),
      Self::StepNext => style(10.5, FontWeight::Normal, 1.0, 0.0, palette.text_muted),
      Self::Initial => style(10.0, FontWeight::SemiBold, 1.0, 0.0, palette.text_secondary),
      Self::Eyebrow => mono(style(10.5, FontWeight::Normal, 1.35, 0.6, palette.text_muted)),
      Self::SecretName => mono(style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary)),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportSpacing {
  /// 18: Create drafts' body over its head and between its parts (`fokEU`).
  Ready,
  /// 20: between Create drafts' sections (`zuLiO`).
  Sections,
  /// 36: the resource list's headings past a row's checkbox (`LZuTH`).
  CheckColumn,
  /// 9: over and under a row of what is left unresolved (`Not imported row` `fuWWh`).
  MissingRow,
}

impl ImportSpacing {
  pub const ALL: [Self; 4] = [Self::Ready, Self::Sections, Self::CheckColumn, Self::MissingRow];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Ready => "import_ready",
      Self::Sections => "import_sections",
      Self::CheckColumn => "import_check_column",
      Self::MissingRow => "import_missing_row",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Ready => 18.0,
      Self::Sections => 20.0,
      Self::CheckColumn => 36.0,
      Self::MissingRow => 9.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in ImportSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ImportText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
