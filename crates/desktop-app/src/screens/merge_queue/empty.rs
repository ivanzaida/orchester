//! An empty merge queue (`Empty queue` `i59Cus` in `mtrlT`): the git-merge tile, *Nothing in the merge queue* and when a
//! run joins it, then how each of the project's published pipelines delivers: its Deliver step's mode and merge
//! method, or that it has none, with *Add a Deliver step*, which opens the pipeline's editor on its Deliver step.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  resource::{DeliverMode, DeliverStep},
};
use orchester_workspace::merge_queue::PipelineDelivery;

use super::words;
use crate::{
  navigation::Navigation,
  routes::{PipelinePanel, ProjectPage, Route},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    state::glyph_tile,
  },
};

/// The height of a pipeline's line in the list (`Pipeline · Feature delivery v3` in `lFDcF`).
const PIPELINE_HEIGHT: f32 = 44.0;

pub(super) fn empty(ctx: &mut Ctx, project: ProjectId, pipelines: &[PipelineDelivery]) -> Column {
  Column::new()
    .id("merge-queue-empty")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Roomy.role())
    .spacing(ExtraSpacing::Inset.role())
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(glyph_tile("merge-queue-empty", Icon::GitMerge))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xs)
        .child(Text::new(&ctx.t("delivery.queue.empty.title")).variant(TypographyStyle::Title))
        .child(
          Text::new(&ctx.t("delivery.queue.empty.body"))
            .variant(ExtraTypography::Lead.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .child(pipelines_list(ctx, project, pipelines))
}

fn pipelines_list(ctx: &mut Ctx, project: ProjectId, pipelines: &[PipelineDelivery]) -> Column {
  let list: Element = if pipelines.is_empty() {
    Text::new(&ctx.t("delivery.queue.empty.noPipelines"))
      .variant(ExtraTypography::Prose.role())
      .into()
  } else {
    let mut list = Column::new()
      .id("merge-queue-pipelines")
      .width(Dimension::Pct(100.0))
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(ExtraRadius::Tile.role());
    for pipeline in pipelines {
      list = list.child(pipeline_line(ctx, project, pipeline));
    }
    list.into()
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&ctx.t("delivery.queue.empty.pipelines"))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted),
    )
    .child(list)
}

fn pipeline_line(ctx: &mut Ctx, project: ProjectId, pipeline: &PipelineDelivery) -> Row {
  let id = format!("merge-queue-pipeline-{}", pipeline.id);
  let name = ctx.t_args(
    "delivery.queue.empty.pipeline",
    [("name", pipeline.name.clone()), ("version", pipeline.version.to_string())],
  );
  let (glyph, words, color) = match &pipeline.deliver {
    Some(deliver) => (Icon::GitMerge, delivers(ctx, deliver), PaletteColor::TextSecondary),
    None => (Icon::CircleDashed, ctx.t("delivery.queue.empty.none"), PaletteColor::TextMuted),
  };
  let mut line = Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PIPELINE_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Md, color.clone()))
    .child(Text::new(&name).variant(ExtraTypography::Strong.role()))
    .child(
      Text::new(&words)
        .variant(ExtraTypography::Prose.role())
        .color(color)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  if pipeline.deliver.is_none() {
    let route = Route::Project(project, ProjectPage::Pipeline(pipeline.id, PipelinePanel::Deliver));
    line = line.child(
      ActionButton::new(format!("{id}-add-deliver"), ButtonKind::Secondary, ctx.t("delivery.queue.empty.addDeliver"))
        .icon(Icon::Plus)
        .opens(Navigation::of(ctx).to(route)),
    );
  }
  line
}

/// *Deliver: approve to merge · merge commit*.
fn delivers(ctx: &Ctx, deliver: &DeliverStep) -> std::sync::Arc<str> {
  let key = match deliver.mode() {
    DeliverMode::ApproveToMerge => "delivery.queue.empty.approve",
    DeliverMode::AutoMerge => "delivery.queue.empty.auto",
  };
  ctx.t_args(key, [("method", words::method(ctx, deliver.method()))])
}
