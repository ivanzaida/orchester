//! The board in rows (swimlanes, `10 Swimlanes` `i96kI`): the columns' heads once at the top, then each row of a
//! module or a priority under its head, 30 high on a `border` line (its chevron, which folds it, its 13 icon, its
//! name in 12/500 `text_secondary`, its count in 12 `text_muted` and how many of its cards need the person), and its
//! cards in the columns, 10 under them. A folded row keeps its head, which then counts its cards in each column
//! (`Lane · No module` `X3AyLx`). The rows scroll together.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  actions::BoardState,
  column::{HeadIn, Shared, column, head, needs_you},
  model::{ColumnModel, LaneModel},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// A row's head (`Lane · Webhooks` `zdAiH`).
const LANE_HEAD: f32 = 30.0;

/// The columns' heads, then every row with its cards.
pub fn lanes(
  ctx: &mut Ctx,
  (state, shared): (&BoardState, &Shared<'_>),
  columns: &[ColumnModel],
  lanes: &[LaneModel],
) -> Element {
  let heads = Row::new()
    .id("tasks-board-heads")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .with_children(columns.iter().map(|column| {
      let id = format!("tasks-board-column-{}", column.key.name());
      Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(head(
        ctx,
        (state, shared),
        column,
        (&id, HeadIn::Lanes),
      ))
    }));
  let mut rows = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role());
  for lane in lanes {
    let folded = shared.display.folded.contains(&lane.key);
    rows = rows.child(lane_head(ctx, state, (lane, columns), folded));
    if folded {
      continue;
    }
    let cells = Row::new()
      .id(format!("tasks-board-lane-{}-cells", lane.key))
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().bottom(ExtraSpacing::Loose.role()))
      .spacing(ExtraSpacing::Loose.role())
      .with_children(
        columns
          .iter()
          .zip(&lane.cells)
          .map(|(column_model, cards)| column(ctx, (state, shared), (column_model, cards), &lane.key)),
      );
    rows = rows.child(cells);
  }
  Column::new()
    .id("tasks-board-lanes")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .spacing(ExtraSpacing::Snug.role())
    .child(heads)
    .child(ScrollVertical::new(rows).flex(1.0).width(Dimension::Pct(100.0)))
    .into()
}

/// A row's head, which folds the row or unfolds it; a folded one counts its cards in each column.
fn lane_head(ctx: &Ctx, state: &BoardState, (lane, columns): (&LaneModel, &[ColumnModel]), folded: bool) -> Element {
  let id = format!("tasks-board-lane-{}", lane.key);
  let (display, key) = (state.display.clone(), lane.key.clone());
  let summary = folded.then(|| {
    let parts: Vec<String> = columns
      .iter()
      .zip(&lane.cells)
      .filter(|(_, cards)| !cards.is_empty())
      .map(|(column, cards)| {
        ctx
          .t_args(
            "tasks.board.lane.inColumn",
            [("count", cards.len().to_string()), ("column", column.label.to_string())],
          )
          .to_string()
      })
      .collect();
    Text::new(&parts.concat())
      .id(format!("{id}-summary"))
      .variant(ExtraTypography::Note.role())
      .nowrap()
  });
  pressable(move || display.update(|display| display.toggle_folded(&key)))
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LANE_HEAD))
    .padding(Padding::horizontal(SpacingSize::Xs))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .describe("state", if folded { "folded" } else { "open" })
    .child(icon(
      if folded { Icon::ChevronRight } else { Icon::ChevronDown },
      IconSize::Sm,
      PaletteColor::TextMuted,
    ))
    .child(icon(lane.glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&lane.label)
        .id(format!("{id}-name"))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(
      Text::new(&lane.count().to_string())
        .id(format!("{id}-count"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .with_children((lane.needs_you > 0).then(|| needs_you(ctx, lane.needs_you, &format!("{id}-needs-you"))))
    .with_children(summary)
    .into()
}
