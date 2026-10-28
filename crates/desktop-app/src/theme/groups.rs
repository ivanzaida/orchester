//! Values of grouped rows (`ui::row_group`): a titled group of rows in one outline, as the redesigned Pipelines
//! screens draw their lists and settings (`Published` `pnnmh` in `x1lmXb`, `Step` `E3MuZQ` in `IsU5u`).

use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupText {
  /// 12/600 `text_secondary`: a group's title over its rows (`m9gK0`).
  Title,
  /// 12 `text_muted`: a group's count beside its title (`gTbNy`), a row's hint (`XkuLQ`) and the words under a
  /// group (`go3qz`).
  Hint,
  /// Geist Mono 13/500 `text_primary`: a row whose label is a path (`xKL70`).
  MonoLabel,
  /// 12.5 `text_secondary`: a value at a row's end (`VI3kR`, `Li31M`).
  Value,
  /// Geist Mono 12.5 `text_secondary`: a value that is an identifier, e.g. a slug or a model (`cNIa0`).
  MonoValue,
  /// 10.5/500 `accent_text`: a tag beside a label, e.g. *Changed* (`ci2Mr`).
  Tag,
}

impl GroupText {
  pub const ALL: [Self; 6] = [
    Self::Title,
    Self::Hint,
    Self::MonoLabel,
    Self::Value,
    Self::MonoValue,
    Self::Tag,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Title => "group_title",
      Self::Hint => "group_hint",
      Self::MonoLabel => "group_mono_label",
      Self::Value => "group_value",
      Self::MonoValue => "group_mono_value",
      Self::Tag => "group_tag",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    let accent = palette.get(super::ExtraColor::AccentText.role());
    match self {
      Self::Title => style(12.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_secondary),
      Self::Hint => style(12.0, FontWeight::Normal, 1.4, 0.0, palette.text_muted),
      Self::MonoLabel => mono(style(13.0, FontWeight::Medium, 1.3, 0.0, palette.text_primary)),
      Self::Value => style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::MonoValue => mono(style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::Tag => style(10.5, FontWeight::Medium, 1.3, 0.0, accent),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  GroupText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
