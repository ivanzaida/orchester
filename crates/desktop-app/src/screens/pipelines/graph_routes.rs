//! Return connectors run outside each parallel row and never take pointer input (dcqi3 / yWz44).
use std::collections::BTreeMap;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Stack},
  node::{HitTestBehavior, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{PipelineEdit, StepKey};

use super::parts;
use crate::{
  theme::{ExtraRadius, ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

type Positions = BTreeMap<StepKey, (f32, f32, f32)>;
const CARD_WIDTH: f32 = 240.0;
const LANE_GAP: f32 = 38.0;
const LABEL_WIDTH: f32 = 132.0;

pub fn routes(ctx: &Ctx, mut graph: Stack, shown: &PipelineEdit, positions: &Positions) -> Stack {
  let mut lanes = [0, 0];
  for step in shown.steps() {
    let Some(route) = &step.on_failure else {
      continue;
    };
    let (Some(&(sx, sy, sh)), Some(&(tx, ty, th))) = (positions.get(&step.key), positions.get(&route.return_to)) else {
      continue;
    };
    let row: Vec<_> = positions.values().filter(|(_, y, _)| *y == sy).collect();
    let left = row.iter().any(|(x, ..)| *x > sx);
    let side = usize::from(!left);
    let extent = positions
      .values()
      .filter(|(_, y, _)| *y >= sy.min(ty) && *y <= sy.max(ty))
      .map(|(x, ..)| if left { *x } else { *x + CARD_WIDTH })
      .reduce(|a, b| if left { a.min(b) } else { a.max(b) })
      .unwrap_or(sx);
    let lane = extent + if left { -LANE_GAP } else { LANE_GAP } * (lanes[side] + 1) as f32;
    lanes[side] += 1;
    let from = (sx + if left { 0.0 } else { CARD_WIDTH }, sy + sh / 2.0);
    let to = (tx + if left { 0.0 } else { CARD_WIDTH }, ty + th / 2.0);
    let mut overlay = Stack::new()
      .id(format!("pipeline-failure-route-{}", step.key))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .hit_test(HitTestBehavior::None);
    overlay = segment(ctx, overlay, from, (lane, from.1));
    overlay = segment(ctx, overlay, (lane, from.1), (lane, to.1));
    overlay = segment(ctx, overlay, (lane, to.1), to);
    overlay = overlay.child(
      Column::new()
        .child(icon(
          if left { Icon::ChevronRight } else { Icon::ChevronLeft },
          IconSize::Xs,
          PaletteColor::TextMuted,
        ))
        .absolute_position(to.0 - 5.0, to.1 - 5.0),
    );
    overlay = overlay.child(
      Column::new()
        .child(
          Row::new()
            .height(Dimension::Px(22.0))
            .padding(Padding::horizontal(SpacingSize::Sm))
            .spacing(ExtraSpacing::Pair.role())
            .align_items(lurq::layout::Alignment::Center)
            .rounded(ExtraRadius::Full.role())
            .background(PaletteColor::SurfacePanel)
            .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
            .child(icon(Icon::Undo2, IconSize::Sm, PaletteColor::TextMuted))
            .child(parts::caption(&ctx.t("pipelines.changesRequested"))),
        )
        .absolute_position(lane - LABEL_WIDTH / 2.0, (from.1 + to.1) / 2.0 - 11.0),
    );
    graph = graph.child(overlay);
  }
  graph
}
fn segment(ctx: &Ctx, graph: Stack, from: (f32, f32), to: (f32, f32)) -> Stack {
  let stroke = ctx.theme().border_size_value(BorderSize::Sm);
  graph.child(
    Column::new()
      .child(
        Row::new()
          .width(Dimension::Px((to.0 - from.0).abs().max(stroke)))
          .height(Dimension::Px((to.1 - from.1).abs().max(stroke)))
          .background(PaletteColor::TextMuted),
      )
      .absolute_position(from.0.min(to.0), from.1.min(to.1)),
  )
}
