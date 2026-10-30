//! Values of Delivery (ORC-19, the `Delivery` layer `p9aqZc`): the Merge queue screen (`seRYH`, `H7OHPT`, `mtrlT`)
//! with its lanes (ORC-410, 05 `S1pTL` of `Base branch · redesign`) and their rows, the pipeline editor's Deliver step
//! (`qukzW`, `OqgbW`), the task page's delivery (`ulEuh`, `RRk9E`, `lvupO`, `wN5Bz`, `hKct7`, `fAcWt`) with `Conflict
//! file row` `Nanwh`, `Commit row` `wEWkY` and `Delivery check row` `QyqsJ`, and its Needs you cards (`K1d1mi`).

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryText {
  /// 13 `text_secondary`, 1.4: what the Merge queue screen says under its title (`kJavK`).
  Lead,
  /// 12.5/500 `text_primary`: a queue row's state (`State label` in `kcUhy`), colored by the state.
  State,
  /// Geist Mono 12 `text_muted`: a section's count (`HKJV1`) and a row's task key (`Key` in `kcUhy`).
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
  /// Geist Mono 13/600 `text_primary`: the base branch in a lane's head (`TXA1k` in `BBL6x`).
  LaneBranch,
}

impl DeliveryText {
  pub const ALL: [Self; 10] = [
    Self::Lead,
    Self::State,
    Self::Count,
    Self::File,
    Self::Branch,
    Self::CardTitle,
    Self::Stage,
    Self::ReadBack,
    Self::Chip,
    Self::LaneBranch,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Lead => "delivery_lead",
      Self::State => "delivery_state",
      Self::Count => "delivery_count",
      Self::File => "delivery_file",
      Self::Branch => "delivery_branch",
      Self::CardTitle => "delivery_card_title",
      Self::Stage => "delivery_stage",
      Self::ReadBack => "delivery_read_back",
      Self::Chip => "delivery_chip",
      Self::LaneBranch => "delivery_lane_branch",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Lead => style(13.0, FontWeight::Normal, 1.4, 0.0, palette.text_secondary),
      Self::State => style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
      Self::Count => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::File => mono(style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::Branch => mono(style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_primary)),
      Self::CardTitle => style(14.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Stage => mono(style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::ReadBack => style(11.0, FontWeight::Normal, 1.4, 0.0, palette.text_muted),
      Self::Chip => style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::LaneBranch => mono(style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary)),
    }
  }
}

/// A switch whose value Auto-merge fixes, with its lock: track and lock faded together (`uKUWl` in `OqgbW`).
pub const LOCKED_SWITCH_OPACITY: f32 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliverySpacing {
  /// 20: between the parts of the Merge queue screen while it is read (`zMx0E`).
  Sections,
  /// 26: a file under the check that found it, in from the check's glyph (`File line` `mjqx2` in `e7geY3`).
  FileIndent,
  /// 14: the Merge queue's info line, its lanes and its note apart, and under the hub's tabs (`YfZEw` in `S1pTL`).
  Lanes,
  /// 10: above a lane's head, and between its glyph, *Into*, its branch, its count and how it lands (`BBL6x`).
  LaneHead,
  /// 14: between a lane row's place, glyph, words, time and actions (`mTyPp`).
  LaneRow,
}

impl DeliverySpacing {
  pub const ALL: [Self; 5] = [
    Self::Sections,
    Self::FileIndent,
    Self::Lanes,
    Self::LaneHead,
    Self::LaneRow,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Sections => "delivery_sections",
      Self::FileIndent => "delivery_file_indent",
      Self::Lanes => "delivery_lanes",
      Self::LaneHead => "delivery_lane_head",
      Self::LaneRow => "delivery_lane_row",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Sections => 20.0,
      Self::FileIndent => 26.0,
      Self::Lanes | Self::LaneRow => 14.0,
      Self::LaneHead => 10.0,
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
