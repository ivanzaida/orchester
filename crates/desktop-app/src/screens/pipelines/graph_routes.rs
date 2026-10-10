//! Return connectors outside the cards (dcqi3 / yWz44).
use std::collections::BTreeMap;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Row, Stack},
  node::{border::Border, dimension::Dimension},
};
use orchester_core::resource::{PipelineEdit, StepKey};

use super::parts;
type Positions = BTreeMap<StepKey, (f32, f32, f32)>;
pub fn routes(ctx: &Ctx, mut graph: Stack, shown: &PipelineEdit, positions: &Positions) -> Stack {
  for step in shown.steps() {
    let Some(route) = &step.on_failure else {
      continue;
    };
    let (Some(&(sx, sy, sh)), Some(&(tx, ty, th))) = (positions.get(&step.key), positions.get(&route.return_to)) else {
      continue;
    };
    let right = sx.max(tx) + 272.0;
    let source = sy + sh / 2.0;
    let target = ty + th / 2.0;
    let stroke = || Border::inside(BorderSize::Sm, PaletteColor::TextMuted);
    graph = graph
      .child(
        lurq::components::Column::new()
          .child(crate::ui::icon::icon(
            crate::ui::icon::Icon::ChevronLeft,
            crate::theme::IconSize::Xs,
            PaletteColor::TextMuted,
          ))
          .absolute_position(tx + 236.0, target - 5.0),
      )
      .child(
        lurq::components::Column::new()
          .child(
            Row::new()
              .width(Dimension::Px(right - sx - 240.0))
              .height(Dimension::Px(ctx.theme().border_size_value(BorderSize::Sm)))
              .border_top(stroke()),
          )
          .absolute_position(sx + 240.0, source),
      )
      .child(
        lurq::components::Column::new()
          .child(
            Row::new()
              .height(Dimension::Px((source - target).abs()))
              .width(Dimension::Px(ctx.theme().border_size_value(BorderSize::Sm)))
              .border_right(stroke()),
          )
          .absolute_position(right, source.min(target)),
      )
      .child(
        lurq::components::Column::new()
          .child(
            Row::new()
              .width(Dimension::Px(right - tx - 240.0))
              .height(Dimension::Px(ctx.theme().border_size_value(BorderSize::Sm)))
              .border_top(stroke()),
          )
          .absolute_position(tx + 240.0, target),
      )
      .child(
        lurq::components::Column::new()
          .child(
            Row::new()
              .padding(SpacingSize::Xs)
              .rounded(RadiusSize::Sm)
              .background(PaletteColor::SurfacePanel)
              .border(stroke())
              .child(parts::caption(&ctx.t("pipelines.changesRequested"))),
          )
          .absolute_position(right - 66.0, (source + target) / 2.0),
      );
  }
  graph
}
