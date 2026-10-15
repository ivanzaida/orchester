//! The design values of the code viewer (layer Files, `o1Uea4` and its states) that the UI kit's roles do
//! not carry, registered as lurq `Extra` roles. Its code, line numbers and path use the Changes tab's roles
//! ([`super::CodeText`], `TypographyStyle::Mono`, `ExtraTypography::MonoNote`).

use std::sync::Arc;

use lurq::{
  app::theme::{RadiusSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// Text of the code viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilesText {
  /// Geist Mono 17/600 `text_primary`: a path the page refuses, as its title (`smYzv` in `S3Py8v`).
  RefusedPath,
  /// 12.5 `text_secondary`: a file's name *Go to file* found (`pZvb2` in `y29f1V`).
  Name,
  /// 12.5/600 `text_primary`: the part of that name that answers the search (`wgY4q`).
  NameMatch,
}

impl FilesText {
  pub const ALL: [Self; 3] = [Self::RefusedPath, Self::Name, Self::NameMatch];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::RefusedPath => "files_refused_path",
      Self::Name => "files_name",
      Self::NameMatch => "files_name_match",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::RefusedPath => mono(style(17.0, FontWeight::SemiBold, 1.25, 0.0, palette.text_primary)),
      Self::Name => style(12.5, FontWeight::Normal, 1.25, 0.0, palette.text_secondary),
      Self::NameMatch => style(12.5, FontWeight::SemiBold, 1.25, 0.0, palette.text_primary),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  FilesText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

/// Radii of the code viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilesRadius {
  /// 2: a place the search within the file found (`Match` `o7yqp`, `Current match` `BAz5o` in `b7RGL`).
  Found,
}

impl FilesRadius {
  pub const ALL: [Self; 1] = [Self::Found];

  pub fn role(self) -> RadiusSize {
    RadiusSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Found => "files_found",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Found => 2.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for radius in FilesRadius::ALL {
    theme.set_radius_value(radius.role(), radius.value());
  }
}
