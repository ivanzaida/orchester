//! A pipeline's page while it is read (`02 Pipeline · loading` `T0MD3`): `← Pipelines`, the tabs and the step column's
//! group headings and row labels stay real; the name, the version line, the tab counts, the flow's step cards and
//! every value and control of the step column are bones, and Run and Publish are bones of their size until the
//! version is known. *Reading pipelines…* (`pipelines.reading`) names it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::{ProjectId, ResourceId};

use super::{flow::FLOW_WIDTH, parts, state::Tab};
use crate::{
  navigation::Navigation,
  routes::{PipelineView, ProjectPage, Route},
  theme::{ExtraSpacing, IconSize, PipelinesText},
  ui::{
    icon::{Icon, icon},
    pressable::Press,
    skeleton::{self, BoneRow, Skeleton, Words, bone, fading, round_bone},
    tabs::{PageTab, page_tabs},
  },
};

/// The name's bone, Run's and Publish's (`Header` in `T0MD3`).
const NAME: f32 = 228.0;
const RUN: f32 = 92.0;
const PUBLISH: f32 = 112.0;
/// A step card's bones: its number, name and two lines (`Step card` in `T0MD3`).
const CARDS: [(f32, f32, f32); 4] = [(44.0, 222.0, 118.0), (28.0, 172.0, 116.0), (40.0, 220.0, 70.0), (40.0, 128.0, 224.0)];
/// A select's or a field's frame in the step column, and its value's bone.
const CONTROL: f32 = 240.0;

/// The page of the pipeline `pipeline` of `project` while it is read, its tab `tab` chosen.
pub fn page(ctx: &mut Ctx, (project, pipeline): (ProjectId, ResourceId), tab: Tab) -> Element {
  let nav = Navigation::of(ctx);
  let open = |view: PipelineView| nav.to(Route::Project(project, ProjectPage::Pipeline(pipeline, view)));
  let back = parts::back_link(
    "pipeline-parent",
    &ctx.t("pipelines.title"),
    Press::Open(nav.to(Route::Project(project, ProjectPage::Pipelines))),
  );
  let title = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(skeleton::line(ctx, TypographyStyle::Heading, NAME))
    .child(Spacer::new().flex(1.0))
    .child(skeleton::button(RUN))
    .child(skeleton::button(PUBLISH));
  let version = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(round_bone(IconSize::Base.size()))
    .child(skeleton::line(ctx, PipelinesText::Status.role(), 76.0))
    .child(skeleton::line(ctx, TypographyStyle::Description, 220.0));
  let tabs = page_tabs(
    ctx,
    "pipeline-tabs",
    [
      PageTab::to("pipeline-tab-steps", ctx.t("pipelines.steps"), tab == Tab::Steps, open(PipelineView::Steps))
        .count_reading(),
      PageTab::to(
        "pipeline-settings",
        ctx.t("pipelines.settingsTab"),
        tab == Tab::Settings,
        open(PipelineView::Settings),
      ),
      PageTab::to(
        "pipeline-history",
        ctx.t("pipelines.versions"),
        tab == Tab::Versions,
        open(PipelineView::Versions),
      )
      .count_reading(),
    ],
  );
  let head = parts::header().child(back).child(title).child(version).child(tabs);
  let steps = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Section)
    .align_items(Alignment::Start)
    .child(flow(ctx))
    .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(column(ctx)));
  let body = Skeleton::new("pipeline-loading", ctx.t("pipelines.reading")).show(ctx, steps);
  parts::page().child(head).child(parts::body("pipeline-body", body)).into()
}

/// The flow: *ON TASK START* real, then the step cards in bones, the lower ones fading.
fn flow(ctx: &Ctx) -> Column {
  let start = Row::new()
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Xs))
    .spacing(ExtraSpacing::Snug.role())
    .rounded(crate::theme::ExtraRadius::Full.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .child(icon(Icon::Play, IconSize::Xs, PaletteColor::TextMuted))
    .child(Text::new(&ctx.t("pipelines.onStart")).variant(PipelinesText::FlowLabel.role()));
  let cards = CARDS
    .iter()
    .map(|(number, name, line)| {
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
        .spacing(SpacingSize::Sm)
        .rounded(RadiusSize::Lg)
        .border_inside(BorderSize::Sm, PaletteColor::Border)
        .child(bone(*number, 9.0))
        .child(skeleton::line(ctx, crate::theme::ExtraTypography::Strong.role(), *name))
        .child(skeleton::line(ctx, TypographyStyle::Description, *line))
    })
    .collect();
  Column::new()
    .id("pipeline-flow-reading")
    .width(Dimension::Px(FLOW_WIDTH))
    .flex_shrink(0.0)
    .padding(Padding::new().top(ExtraSpacing::Tight.role()).bottom(SpacingSize::Xl))
    .spacing(SpacingSize::Md)
    .child(start)
    .with_children(fading(cards))
}

/// The step column: the step's name field, then its groups with their row labels real and every value a bone.
fn column(ctx: &Ctx) -> Column {
  let select = |ctx: &Ctx, value| skeleton::field(ctx, CONTROL, value);
  let labelled = |id: &str, key: &str| BoneRow::new(id.to_owned(), Words::Real(ctx.t(key)));
  let step = vec![
    labelled("pipeline-reading-executor", "pipelines.executor")
      .hint(Words::Bone(150.0))
      .end(select(ctx, 130.0))
      .build(ctx)
      .into(),
    labelled("pipeline-reading-model", "pipelines.model")
      .end(select(ctx, 150.0))
      .build(ctx)
      .into(),
    labelled("pipeline-reading-instructions", "pipelines.instructions")
      .hint(Words::Bone(276.0))
      .end(skeleton::chevron())
      .build(ctx)
      .into(),
  ];
  let flow = vec![
    labelled("pipeline-reading-after", "pipelines.after")
      .end(select(ctx, 96.0))
      .build(ctx)
      .into(),
    labelled("pipeline-reading-failure", "pipelines.failure")
      .end(select(ctx, 116.0))
      .build(ctx)
      .into(),
    labelled("pipeline-reading-budget", "pipelines.budget")
      .end(select(ctx, 46.0))
      .build(ctx)
      .into(),
  ];
  let access = vec![
    labelled("pipeline-reading-gate", "pipelines.gate")
      .end(skeleton::switch())
      .build(ctx)
      .into(),
    labelled("pipeline-reading-access", "pipelines.access")
      .end(skeleton::button(144.0))
      .build(ctx)
      .into(),
    labelled("pipeline-reading-base", "pipelines.baseMoves")
      .end(skeleton::switch())
      .build(ctx)
      .into(),
    labelled("pipeline-reading-sources", "pipelines.sources")
      .hint(Words::Bone(100.0))
      .end(skeleton::chevron())
      .build(ctx)
      .into(),
    labelled("pipeline-reading-permissions", "pipelines.permissions")
      .hint(Words::Bone(100.0))
      .end(skeleton::chevron())
      .build(ctx)
      .into(),
  ];
  let real = |key: &str| (Words::Real(ctx.t(key)), false);
  let groups = fading(vec![
    skeleton::group(ctx, "pipeline-reading-step", real("pipelines.stepGroup"), None, step),
    skeleton::group(ctx, "pipeline-reading-flow", real("pipelines.flow"), None, flow),
    skeleton::group(ctx, "pipeline-reading-gate-access", real("pipelines.gateAccess"), None, access),
  ]);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(bone(18.0, 9.0))
        .child(skeleton::field(ctx, 284.0, 104.0)),
    )
    .with_children(groups)
}
