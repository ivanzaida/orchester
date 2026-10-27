//! What a drag draws over a column, so no card moves (`Drag overlay` `wjtLk` in `03 Drop refused` `bR7zd`, `02
//! Dragging` `goJvp`): Done's two drop zones over its cards, *Mark as done* over the top half and, while Canceled is
//! hidden, *Cancel task* under it, 6 apart, each veiled by `surface_base` at 82 % in a `border_strong` stroke, radius
//! 6, its 14 mark and 12.5/500 `text_secondary` name centred 12 under its top; the one under the pointer raised on
//! `surface_raised` in a 2 `border_focus` stroke (`Drop zone · Mark as done` `GdDT9`, `Drop zone · Cancel task`
//! `XzRhw`). A column that takes the card shows where it would go as a 2 `border_focus` line between its cards
//! (`Insertion line` `gPQ23`), drawn across the gap over the card it goes before, or under the last one.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Stack, Text},
  layout::{Alignment, StackAlignment, layout_kind::Justify},
  node::{Element, HitTestBehavior, border::Border, dimension::Dimension, padding::Padding, transform::Transform2D},
};
use orchester_core::task::organization::WorkflowStatus;

use super::{
  actions::{BoardState, Refusal, Target},
  card::CardModel,
  column::Shared,
  drop::refusal_of,
  model::ColumnKey,
  refusal::{self, Shown},
};
use crate::{
  theme::{DROP_VEIL_OPACITY, ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// The insertion line's height (`gPQ23`).
const LINE: f32 = 2.0;

/// How a column looks while a card is lifted, or after a refused move: lit in the tone of what it does with the
/// card, what its overlay layer shows, and where the insertion line goes among its cards.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feedback {
  pub lit: Option<Lit>,
  pub layer: Option<Layer>,
  pub insertion: Option<Insertion>,
}

/// A column the pointer is over: it takes the card (`accent_muted` in a `border_strong` stroke, `Column · Todo` in
/// `goJvp`), or refuses it (`danger_muted` in a `danger` stroke, `Column · In review` `I6T2n`). Done is lit so for a
/// card whose run has not ended wherever it is over (`Column · Done` `y2yvT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lit {
  Takes,
  Refuses,
}

/// What a column's overlay layer shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Layer {
  Refused(Refusal, Shown),
  Zones(Zones),
  /// The insertion line over the top of its cards.
  TopLine,
}

/// How the column `target` holding `cards` looks for what `shared` says: the column a card was lifted from keeps its
/// place as an outline and nothing more.
pub fn feedback(shared: &Shared<'_>, (target, cards): (&Target, &[CardModel])) -> Feedback {
  let Some(drag) = shared.drag else {
    // After a refused drop or move, the column says why until the next one.
    return Feedback {
      layer: shared
        .refused
        .filter(|refused| refused.column == target.column)
        .map(|refused| {
          let shown = Shown {
            hovered: true,
            scrim: false,
          };
          Layer::Refused(refused.refusal.clone(), shown)
        }),
      ..Feedback::default()
    };
  };
  if drag.from.column == target.column {
    return Feedback::default();
  }
  let over = drag.over.as_ref() == Some(target);
  if let Some(refusal) = refusal_of(&drag.card, target) {
    return Feedback {
      lit: over.then_some(Lit::Refuses),
      layer: Some(Layer::Refused(
        refusal,
        Shown {
          hovered: over,
          scrim: true,
        },
      )),
      insertion: None,
    };
  }
  if target.column == ColumnKey::Status(WorkflowStatus::Done) {
    let cancel = (!shared.display.show_canceled).then(|| Target {
      column: ColumnKey::Status(WorkflowStatus::Canceled),
      lane: target.lane.clone(),
    });
    let over_cancel = cancel.as_ref().is_some_and(|zone| drag.over.as_ref() == Some(zone));
    // A card whose run has not ended goes only to Done or Canceled: Done offers both wherever it is over (`bR7zd`).
    if !(drag.card.row.run_not_ended || over || over_cancel) {
      return Feedback::default();
    }
    let zones = Zones {
      done: over,
      cancel: cancel.map(|zone| (zone, over_cancel)),
    };
    return Feedback {
      lit: Some(Lit::Takes),
      layer: Some(Layer::Zones(zones)),
      insertion: None,
    };
  }
  if !over {
    return Feedback::default();
  }
  // Order in a column is task number order: the card goes before the first with a higher number.
  let number = key_number(&drag.card.row.key);
  let at = cards
    .iter()
    .position(|card| key_number(&card.row.key) > number)
    .unwrap_or(cards.len());
  let insertion = Insertion::at(at, cards.len());
  Feedback {
    lit: Some(Lit::Takes),
    layer: (insertion == Insertion::Top).then_some(Layer::TopLine),
    insertion: (insertion != Insertion::Top).then_some(insertion),
  }
}

/// The overlay layer of the column `id`.
pub fn layer(ctx: &mut Ctx, state: &BoardState, id: &str, layer: &Layer) -> Element {
  match layer {
    Layer::Refused(refusal, shown) => refusal::layer(ctx, id, refusal, *shown),
    Layer::Zones(shown) => zones(ctx, state, id, shown),
    Layer::TopLine => top_line(id),
  }
}

/// The number of a task's key (`KON-12` is 12), its order in a column.
fn key_number(key: &str) -> u64 {
  key
    .rsplit_once('-')
    .and_then(|(_, number)| number.parse().ok())
    .unwrap_or(u64::MAX)
}

/// Done's zones while a card is lifted: whether the pointer is over *Mark as done*, and *Cancel task* with whether
/// the pointer is over it, while Canceled is hidden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zones {
  pub done: bool,
  pub cancel: Option<(Target, bool)>,
}

/// Done's zones over the cards of the column `id`; the *Cancel task* zone is a drop target of its own, before the
/// column, so a drop on it cancels the task.
pub fn zones(ctx: &Ctx, state: &BoardState, id: &str, zones: &Zones) -> Element {
  let done = zone(
    ctx,
    &format!("{id}-zone-done"),
    (Icon::CircleCheck, PaletteColor::Success),
    ("task_close.menu.done", zones.done),
  );
  let cancel = zones.cancel.as_ref().map(|(target, over)| {
    zone(
      ctx,
      &format!("{id}-zone-canceled"),
      (Icon::CircleX, PaletteColor::TextSecondary),
      ("task_close.menu.cancel", *over),
    )
    .ref_element(state.target_ref(target.clone()))
  });
  Column::new()
    .id(format!("{id}-overlay"))
    .absolute(0.0, 0.0, Dimension::Pct(100.0), Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .hit_test(HitTestBehavior::None)
    .child(done)
    .with_children(cancel)
    .into()
}

/// One of Done's zones, raised while the pointer is over it.
fn zone(ctx: &Ctx, id: &str, (glyph, color): (Icon, PaletteColor), (key, over): (&str, bool)) -> Column {
  let label = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Md))
    .spacing(ExtraSpacing::Snug.role())
    .justify(Justify::Center)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, color))
    .child(
      Text::new(&ctx.t(key))
        .variant(ExtraTypography::OptionTitle.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    );
  let zone = Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .rounded(RadiusSize::Md)
    .clip()
    .describe("state", if over { "over" } else { "offered" });
  if over {
    return zone
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Lg, PaletteColor::BorderFocus))
      .child(label);
  }
  zone
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .child(
      Column::new()
        .absolute(0.0, 0.0, Dimension::Pct(100.0), Dimension::Pct(100.0))
        .background(PaletteColor::SurfaceBase)
        .opacity(DROP_VEIL_OPACITY),
    )
    .child(label)
}

/// Where the line goes among a column's cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Insertion {
  /// Over the top of the column's cards: before the first, or in an empty column.
  Top,
  /// Across the gap over the card at this place.
  Before(usize),
  /// Across the gap under the last card.
  After,
}

impl Insertion {
  /// Where a card goes among `count` cards when it goes at place `at`.
  pub const fn at(at: usize, count: usize) -> Self {
    match at {
      0 => Self::Top,
      at if at >= count => Self::After,
      at => Self::Before(at),
    }
  }
}

/// The line over the top of a column's cards, in its overlay layer.
pub fn top_line(id: &str) -> Element {
  Column::new()
    .id(format!("{id}-overlay"))
    .absolute(0.0, 0.0, Dimension::Pct(100.0), Dimension::Pct(100.0))
    .hit_test(HitTestBehavior::None)
    .child(line())
    .into()
}

/// `card` with the line across the gap over it (`above`) or under it.
pub fn with_line(card: Element, above: bool) -> Element {
  let shift = (ExtraSpacing::Snug.value() + LINE) / 2.0;
  let (align, shift) = if above {
    (StackAlignment::TopStart, -shift)
  } else {
    (StackAlignment::BottomStart, shift)
  };
  Stack::new()
    .width(Dimension::Pct(100.0))
    .stack_align(align)
    .child(card)
    .child(line().transform(Transform2D::translate(0.0, shift)))
    .into()
}

fn line() -> Row {
  Row::new()
    .id("tasks-board-insertion")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LINE))
    .background(PaletteColor::BorderFocus)
    .hit_test(HitTestBehavior::None)
}
