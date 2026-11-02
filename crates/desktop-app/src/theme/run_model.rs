//! Values of a run's model chip, its picker and its event lines (ORC-322; `Model chip` `VJUdj`, `Model menu` `LNBnH`
//! in 08 `DPofN`, the chip states and event lines of 08f `Yzb3F`) that the UI kit's roles do not carry.

use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelText {
  /// 11.5 `text_secondary`, 1.3: what the chip says (`LZbAC`); *after this turn* after it in `text_muted`.
  Chip,
  /// 11/500 `text_muted`, 1.3: a heading of the picker, *Builder · Claude Code · Work · its models* (`O65tgD`),
  /// *Applies* (`QVMW5`).
  Heading,
  /// Geist Mono 12 `text_primary`, 1.3: a model's id in the picker (`eS4PW`).
  Name,
  /// Geist Mono 11 `text_muted`, 1.3: the *now* at the end of the model in use (`EL5Hv`).
  Tag,
  /// 12 `text_muted`, 1.3: the label before the effort's field (`OU3jd`).
  Label,
  /// 12 `text_primary`, 1.3: the effort chosen in its field (`uL3j1`).
  Value,
  /// 11.5 `text_muted`, 1.4: what an option of *Applies* does (`jmqYl`), the foot's note (`b3pY9`).
  Sub,
}

impl ModelText {
  pub const ALL: [Self; 7] = [
    Self::Chip,
    Self::Heading,
    Self::Name,
    Self::Tag,
    Self::Label,
    Self::Value,
    Self::Sub,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Chip => "model_chip",
      Self::Heading => "model_heading",
      Self::Name => "model_name",
      Self::Tag => "model_tag",
      Self::Label => "model_label",
      Self::Value => "model_value",
      Self::Sub => "model_sub",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Chip => style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::Heading => style(11.0, FontWeight::Medium, 1.3, 0.0, palette.text_muted),
      Self::Name => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary)),
      Self::Tag => mono(style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::Label => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::Value => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::Sub => style(11.5, FontWeight::Normal, 1.4, 0.0, palette.text_muted),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ModelText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
