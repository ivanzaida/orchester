//! Values of the context meter (ORC-305): the meter in a composer's bar and on a step's divider (`Context meter ·
//! states` `Kp4IZ` in `Orchestrator · redesign`, `bxHFm` in `Task page · redesign`) and its popover (`v6vdj`, `fJ1Bn`,
//! `yIbCQ`, `OZGiw`, `yCk2o`).

use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextText {
  /// Geist Mono 11 `text_muted`, 1.3: the share beside the ring, and in the popover (`kYtZG`, `r1wR1`); `warning` near
  /// full, `danger` full.
  Share,
  /// 11 `danger`, 1.3: *Full* in the meter's place of the share (`kqwKb`).
  Full,
  /// 12/600 `text_primary`, 1.3: the popover's *Context* (`C0x35f`).
  Title,
  /// Geist Mono 11 `text_muted`, 1.3: the model, or the agent and its model, at the popover head's end (`hncjV`).
  Model,
  /// 12 `text_primary`, 1.3: *76k of 200k tokens* (`DtYpM`).
  Tokens,
  /// 11 `text_muted`, 1.45: what an estimate or an unknown share means (`ZHx3p`, `YwJOp`), and the hint beside the
  /// popover's *Compact* (`ndDkY`).
  Note,
  /// 11 `text_secondary`, 1.3: *Compacting…* in the meter's place while its session is compacted (`xD1op`).
  Compacting,
}

impl ContextText {
  pub const ALL: [Self; 7] = [
    Self::Share,
    Self::Full,
    Self::Title,
    Self::Model,
    Self::Tokens,
    Self::Note,
    Self::Compacting,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Share => "context_share",
      Self::Full => "context_full",
      Self::Title => "context_title",
      Self::Model => "context_model",
      Self::Tokens => "context_tokens",
      Self::Note => "context_note",
      Self::Compacting => "context_compacting",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Share | Self::Model => mono(style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::Full => style(11.0, FontWeight::Normal, 1.3, 0.0, palette.danger),
      Self::Title => style(12.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Tokens => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::Note => style(11.0, FontWeight::Normal, 1.45, 0.0, palette.text_muted),
      Self::Compacting => style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ContextText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
