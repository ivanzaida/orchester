//! A card about a run's services in its Conversation (`Agent testing · redesign` `eWydp`: `tz1d8` in `FwHrv`,
//! `TienU` in `hvQDS`, `SGntT`, `DPLN1` and `qsBGf` in `oXVuM`): on `surface_raised` in a stroke of its tone
//! (`danger` for a crash or a failed start, `warning` while a start waits for the person, `info` while it waits for
//! slots), radius 10, 14 by 16 inside, its parts 10 apart. Its head is a 14 glyph in the tone, the title in 13/600 and,
//! 8 after it, a meta line in 12 `text_muted` (at the head's end on the approve card). Its body says what happened in
//! 12.5/1.5 `text_secondary`.
//!
//! A log tail is the lines on `code_bg` in a `border` stroke, radius 6, 8 by 10 inside and 3 apart, each its source in
//! Geist Mono 11.5 `text_muted` in a column of 84, 10 before what it says in Geist Mono 11.5 `text_secondary` (standard
//! error and Orchester's own line in `danger`). A slot bar is the slots side by side, as wide as it is given, each 8
//! high, radius 4, 4 apart: `accent` for one another start holds, `warning` for one this start takes, `border` for one
//! that stays free.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::icon::{Icon, icon};
use crate::theme::{CardsText, ExtraColor, ExtraSpacing, IconSize, RunServicesText, ServiceCardsText};

/// The column a log line's source stands in (`Source` `ykqN1` in `tz1d8`).
const SOURCE_WIDTH: f32 = 84.0;
/// A slot's height in a slot bar (`Slot` `cC219` in `TienU`).
const SLOT_HEIGHT: f32 = 8.0;

/// The card's frame, `id`, in the stroke of `tone`, its parts 10 apart.
pub fn frame(id: impl Into<String>, tone: PaletteColor) -> Column {
  Column::new()
    .id(id.into())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, tone))
    .rounded(RadiusSize::Lg)
}

/// Where the head's meta line stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Meta {
  /// 8 after the title (`PwQIU` in `tz1d8`).
  AfterTitle,
  /// At the head's end (`j4MyD` in `TienU`).
  AtEnd,
}

/// The head of the card `id`: `glyph` in `tone`, `title` and `meta` where `place` puts it.
pub fn head(id: &str, (glyph, tone): (Icon, PaletteColor), title: &str, (meta, place): (&str, Meta)) -> Row {
  let title = Text::new(title)
    .id(format!("{id}-title"))
    .variant(CardsText::ApprovalTitle.role())
    .selectable(true)
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0));
  let meta = Text::new(meta)
    .id(format!("{id}-meta"))
    .variant(CardsText::Quiet.role())
    .nowrap();
  let row = Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, tone))
    .child(title);
  match place {
    Meta::AfterTitle => row.child(meta),
    Meta::AtEnd => row.child(Spacer::new().flex(1.0)).child(meta),
  }
}

/// What happened and what Orchester did, `id`.
pub fn body(id: &str, words: &str) -> Text {
  Text::new(words)
    .id(id.to_owned())
    .variant(ServiceCardsText::Body.role())
    .selectable(true)
    .width(Dimension::Pct(100.0))
}

/// One line of a log tail: where it came from, what it says, and whether it says something went wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TailLine {
  pub source: Arc<str>,
  pub text: Arc<str>,
  pub danger: bool,
}

impl TailLine {
  /// A line `service` wrote, as a crash or a failed start keeps it (`<number> <out|err>| <text>`); a row in another
  /// shape is shown whole.
  pub fn of_row(service: &str, row: &str) -> Self {
    let (stream, text) = row
      .split_once("| ")
      .and_then(|(head, text)| Some((head.rsplit_once(' ')?.1, text)))
      .unwrap_or(("out", row));
    Self {
      source: service.into(),
      text: text.into(),
      danger: stream == "err",
    }
  }
}

/// The lines of a log tail, `id`.
pub fn log_tail(id: &str, lines: &[TailLine]) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Fine.role())
    .background(ExtraColor::CodeBackground.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .with_children(lines.iter().enumerate().map(|(at, line)| tail_line(id, at, line)))
}

fn tail_line(id: &str, at: usize, line: &TailLine) -> Element {
  let text = Text::new(&line.text)
    .id(format!("{id}-line-{at}"))
    .variant(RunServicesText::LogLine.role())
    .selectable(true)
    .flex(1.0)
    .min_width(Dimension::Px(0.0));
  let text = if line.danger {
    text.color(PaletteColor::Danger)
  } else {
    text
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .describe("danger", line.danger.to_string())
    .child(
      Text::new(&line.source)
        .variant(RunServicesText::LogTime.role())
        .nowrap()
        .width(Dimension::Px(SOURCE_WIDTH))
        .flex_shrink(0.0),
    )
    .child(text)
    .into()
}

/// A slot of a slot bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
  /// Another start holds it.
  Held,
  /// This start takes it.
  Taken,
  /// It stays free.
  Free,
}

impl Slot {
  const fn fill(self) -> PaletteColor {
    match self {
      Self::Held => PaletteColor::Accent,
      Self::Taken => PaletteColor::Warning,
      Self::Free => PaletteColor::Border,
    }
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Held => "held",
      Self::Taken => "taken",
      Self::Free => "free",
    }
  }
}

/// The slot bar `id`, its `slots` in order, as wide as `width`.
pub fn slot_bar(id: &str, slots: &[Slot], width: Dimension) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(width)
    .spacing(SpacingSize::Xs)
    .with_children(slots.iter().enumerate().map(|(at, slot)| {
      Row::new()
        .id(format!("{id}-{at}"))
        .describe("slot", slot.name())
        .flex(1.0)
        .height(Dimension::Px(SLOT_HEIGHT))
        .rounded(RadiusSize::Sm)
        .background(slot.fill())
    }))
}
