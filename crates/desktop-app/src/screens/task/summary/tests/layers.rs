//! A layer's row on the Tests card (`Layer · lint` `A6Bbso` in `qN2FK`): 9 over and under and 14 at its sides, 12
//! apart, under a `border` line: a 13 glyph (`circle-check` in `success` when it passed, `circle-x` in `danger` when
//! it failed, `circle-dashed` in `text_muted` when a layer before it failed), its name in Geist Mono 12.5/500 in 48,
//! its suites in 12 `text_muted`, its counts in 12, 10 apart (passed in `success`, failed in `danger`, flaky in
//! `warning`; a lint layer that passed has *no problems*), and its time in Geist Mono 12 `text_muted`, at the end of
//! 64.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{resource::TestLayer, test_step::LayerRecord};

use super::words::{counted, duration};
use crate::{
  theme::{ExtraSpacing, IconSize, TestsText},
  ui::icon::{Icon, icon},
};

/// A layer's name (`Name` `Cix9O`).
const NAME_WIDTH: f32 = 48.0;
/// A layer's time (`Time` `I9PYSa`).
const TIME_WIDTH: f32 = 64.0;

pub fn row(ctx: &Ctx, layer: &LayerRecord) -> Element {
  let id = format!("task-tests-layer-{}", layer.layer.as_str());
  let (glyph, tone) = if !layer.ran {
    (Icon::CircleDashed, PaletteColor::TextMuted)
  } else if layer.passed() {
    (Icon::CircleCheck, PaletteColor::Success)
  } else {
    (Icon::CircleX, PaletteColor::Danger)
  };
  let suites: Vec<&str> = layer.suites.iter().map(|suite| suite.suite.as_str()).collect();
  let suites = suites.join(&ctx.t("summary.tests.suiteSeparator"));
  let time = if layer.ran {
    duration(ctx, layer.duration_ms())
  } else {
    ctx.t("summary.tests.noTime")
  };
  Row::new()
    .id(id.clone())
    .describe("passed", layer.passed().to_string())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Inline.role(),
      ExtraSpacing::Inset.role(),
    ))
    .spacing(lurq::app::theme::SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Base, tone))
    .child(
      Row::new().width(Dimension::Px(NAME_WIDTH)).flex_shrink(0.0).child(
        Text::new(layer.layer.as_str())
          .id(format!("{id}-name"))
          .variant(TestsText::Name.role())
          .nowrap(),
      ),
    )
    .child(
      Text::new(&suites)
        .id(format!("{id}-suites"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Row::new()
        .id(format!("{id}-counts"))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .with_children(counts(ctx, layer)),
    )
    .child(
      Row::new()
        .width(Dimension::Px(TIME_WIDTH))
        .flex_shrink(0.0)
        .justify(Justify::End)
        .child(
          Text::new(&time)
            .id(format!("{id}-time"))
            .variant(TestsText::Detail.role())
            .nowrap(),
        ),
    )
    .into()
}

/// What the layer's suites counted, each in its tone.
fn counts(ctx: &Ctx, layer: &LayerRecord) -> Vec<Element> {
  let count = |words: std::sync::Arc<str>, tone: PaletteColor| -> Element {
    Text::new(&words)
      .variant(TypographyStyle::Description)
      .color(tone)
      .nowrap()
      .into()
  };
  if !layer.ran {
    return vec![count(ctx.t("summary.tests.notRun"), PaletteColor::TextMuted)];
  }
  let (failed, flaky, problems) = (layer.failed_tests(), layer.flaky_tests(), layer.suites_with_problems());
  if layer.layer == TestLayer::Lint && layer.passed() && flaky == 0 {
    return vec![count(ctx.t("summary.tests.noProblems"), PaletteColor::Success)];
  }
  let mut counts = vec![count(
    ctx.t_args("summary.tests.passed", [("count", layer.passed_tests().to_string())]),
    PaletteColor::Success,
  )];
  if failed > 0 {
    counts.push(count(
      ctx.t_args("summary.tests.failed", [("count", failed.to_string())]),
      PaletteColor::Danger,
    ));
  }
  if flaky > 0 {
    counts.push(count(
      ctx.t_args("summary.tests.flaky", [("count", flaky.to_string())]),
      PaletteColor::Warning,
    ));
  }
  if problems > 0 {
    let keys = ("summary.tests.problems.one", "summary.tests.problems.other");
    counts.push(count(counted(ctx, keys, problems), PaletteColor::Danger));
  }
  counts
}
