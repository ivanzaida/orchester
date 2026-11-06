//! Values of Delivery (ORC-19, the `Delivery` layer `p9aqZc`): the pipeline editor's Deliver step (`qukzW`, `OqgbW`),
//! the task page's delivery (`ulEuh`, `RRk9E`, `lvupO`, `wN5Bz`, `hKct7`, `fAcWt`) with `Conflict file row` `Nanwh`,
//! `Commit row` `wEWkY` and `Delivery check row` `QyqsJ`, and its Needs you cards (`K1d1mi`).

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryText {
  /// Geist Mono 12 `text_muted`: a count and a task key (`Key` in `kcUhy`).
  Count,
  /// Geist Mono 11.5 `text_secondary`: an uncommitted or conflicting file (`QjQHb`, `src/webhooks/sender.rs`).
  File,
  /// Geist Mono 12.5 `text_primary`: a branch on the review's merge line (`kon-12-webhook-retry` in `bwHYE`).
  Branch,
  /// 14/600 `text_primary`: the title of a delivery card on the task page (`Delivering into main` in `IJVXv`).
  CardTitle,
  /// Geist Mono 11 `text_muted`: a stage's number (`01–06` in `ClizQ`) and a check row's time (`16:58`).
  Stage,
  /// 11 `text_muted`, 1.4: what a Deliver setting does, under it (`eNKak` in `azqkP`).
  ReadBack,
  /// 12.5 `text_primary`: how a pipeline delivers, on the canvas's chip (`UVqgh` in `gURMA`).
  Chip,
}

impl DeliveryText {
  pub const ALL: [Self; 7] = [
    Self::Count,
    Self::File,
    Self::Branch,
    Self::CardTitle,
    Self::Stage,
    Self::ReadBack,
    Self::Chip,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Count => "delivery_count",
      Self::File => "delivery_file",
      Self::Branch => "delivery_branch",
      Self::CardTitle => "delivery_card_title",
      Self::Stage => "delivery_stage",
      Self::ReadBack => "delivery_read_back",
      Self::Chip => "delivery_chip",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Count => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::File => mono(style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::Branch => mono(style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_primary)),
      Self::CardTitle => style(14.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Stage => mono(style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::ReadBack => style(11.0, FontWeight::Normal, 1.4, 0.0, palette.text_muted),
      Self::Chip => style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
    }
  }
}

/// A switch whose value Auto-merge fixes, with its lock: track and lock faded together (`uKUWl` in `OqgbW`).
pub const LOCKED_SWITCH_OPACITY: f32 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliverySpacing {
  /// 26: a file under the check that found it, in from the check's glyph (`File line` `mjqx2` in `e7geY3`).
  FileIndent,
  /// 14: the parts of the attempt page apart (`Review` `ZPIfX` in 09 `Os7b3`).
  Lanes,
  /// 56: the attempt page's empty state under its status line, past the parts' gap (09c `jhPCL`).
  MergesEmpty,
}

impl DeliverySpacing {
  pub const ALL: [Self; 3] = [Self::FileIndent, Self::Lanes, Self::MergesEmpty];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::FileIndent => "delivery_file_indent",
      Self::Lanes => "delivery_lanes",
      Self::MergesEmpty => "delivery_merges_empty",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::FileIndent => 26.0,
      Self::Lanes => 14.0,
      Self::MergesEmpty => 56.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in DeliverySpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  DeliveryText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
