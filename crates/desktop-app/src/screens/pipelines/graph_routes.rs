//! Return connectors run outside each parallel row and never take pointer input (dcqi3 / yWz44).
use std::collections::BTreeMap;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Stack, Svg},
  node::{HitTestBehavior, dimension::Dimension, padding::Padding},
  svg::SvgData,
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
    overlay = overlay.child(connector(ctx, from, to, lane));
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
fn connector(ctx: &Ctx, from: (f32, f32), to: (f32, f32), lane: f32) -> Column {
  let stroke = ctx.theme().border_size_value(crate::theme::pipelines::return_stroke());
  let bend = ctx
    .theme()
    .radius_value(crate::theme::pipelines::return_radius())
    .min((from.1 - to.1).abs() / 2.0);
  let (dx, dy) = ((lane - from.0).signum(), (to.1 - from.1).signum());
  let (left, top) = (from.0.min(to.0).min(lane) - stroke, from.1.min(to.1) - 5.0);
  let (width, height) = (from.0.max(to.0).max(lane) - left + stroke, (to.1 - from.1).abs() + 10.0);
  let path = format!(
    "M{} {} H{} Q{} {} {} {} V{} Q{} {} {} {} H{}",
    from.0,
    from.1,
    lane - dx * bend,
    lane,
    from.1,
    lane,
    from.1 + dy * bend,
    to.1 - dy * bend,
    lane,
    to.1,
    lane - dx * bend,
    to.1,
    to.0
  );
  let outline = format!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="{left} {top} {width} {height}"><path d="{path}" fill="none" stroke="black" stroke-width="{stroke}"/></svg>"#
  );
  let arrow_x = to.0 + dx * 7.0;
  let arrow = format!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="{left} {top} {width} {height}"><path d="M{} {} L{arrow_x} {} L{arrow_x} {} Z"/></svg>"#,
    to.0,
    to.1,
    to.1 - 4.0,
    to.1 + 4.0
  );
  let color = ctx.theme().palette_color(PaletteColor::TextMuted);
  Column::new()
    .child(
      Stack::new()
        .width(Dimension::Px(width))
        .height(Dimension::Px(height))
        .child(
          Svg::new(SvgData::from_str(&outline).with_stroke(color)).size(Dimension::Px(width), Dimension::Px(height)),
        )
        .child(Svg::new(SvgData::from_str(&arrow).with_fill(color)).size(Dimension::Px(width), Dimension::Px(height))),
    )
    .absolute_position(left, top)
}
