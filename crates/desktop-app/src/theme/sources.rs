//! Values of Sources (`XulGo`): the source rows (`Source row` `Q5DCn`, `Source row/compact` `KsqMM`), *Add source*
//! (`t4r78`) and the lists they make on Project settings, the task form, Pipeline settings and the step inspector.
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourcesText {
  /// Geist Mono 12.5 `text_primary`: a source's path in a list (`uixJK` in `Q5DCn`).
  Path,
}

impl SourcesText {
  pub const ALL: [Self; 1] = [Self::Path];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Path => "sources_path",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Path => mono(style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_primary)),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourcesSpacing {
  /// 14: between a Sources section's parts (`oCZK1`), above a task's Sources (`zcFQJ`).
  Section,
  /// 18: above an empty list's title (`Empty` `VXZ0x`).
  EmptyTop,
}

impl SourcesSpacing {
  pub const ALL: [Self; 2] = [Self::Section, Self::EmptyTop];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Section => "sources_section",
      Self::EmptyTop => "sources_empty_top",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Section => 14.0,
      Self::EmptyTop => 18.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in SourcesSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  SourcesText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
