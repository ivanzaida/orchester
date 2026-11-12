//! *Start from* in a new pipeline's panel (`Start from` `eZG0q` in P14 `Rszkt`), which replaces the old page's radio
//! rows: *Starter · Feature delivery* with its steps and approval gates, *Copy a pipeline* with a select of the
//! pipelines (as *name · version*; its latest version becomes v1) and *Empty pipeline*. The chosen row says what v1
//! gets: the starter's chain, or the copy's steps (P14b `sVNK4`). Choosing a pipeline in the select chooses the row.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::Pipeline;
use orchester_workspace::pipelines::levels::levels;

use super::Choice;
use crate::{
  screens::{pipeline_hub::model::Entry, team_hub::rows},
  theme::{ExtraSpacing, ExtraTypography, HubText},
  ui::{field::SelectField, pressable::Press, toggle::radio_mark},
};

/// The copy select's width (`Select` `RPuSL`).
const COPY_WIDTH: f32 = 220.0;

/// What the group is drawn from.
pub(super) struct Start<'a> {
  pub choice: &'a Signal<Choice>,
  pub copy: &'a Signal<String>,
  /// The starter as this project's profiles and tools fill it.
  pub starter: &'a Pipeline,
  /// How many approval gates the starter has.
  pub starter_gates: usize,
  /// The pipelines that can be copied, and the one chosen.
  pub copies: &'a [&'a Entry],
  pub source: Option<&'a Entry>,
  pub locked: bool,
}

pub(super) fn group(ctx: &Ctx, start: &Start<'_>) -> Element {
  let chosen = start.choice.get();
  let lead = Text::new(&ctx.t("pipeline_new.start.lead"))
    .id("pipeline-new-start-lead")
    .variant(HubText::PanelHint.role())
    .nowrap();
  let starter_lines = {
    let key = match start.starter_gates {
      0 => "pipeline_new.start.starterSummaryNoGate",
      1 => "pipeline_new.start.starterSummaryOneGate",
      _ => "pipeline_new.start.starterSummary",
    };
    let summary = ctx.t_args(
      key,
      [
        ("steps", start.starter.steps().len().to_string()),
        ("gates", start.starter_gates.to_string()),
      ],
    );
    let mut lines = vec![(summary.to_string(), PaletteColor::TextSecondary)];
    if chosen == Choice::Starter {
      lines.push((chain(ctx, start.starter), PaletteColor::TextMuted));
    }
    lines
  };
  let rows = vec![
    option(
      ctx,
      start,
      (
        Choice::Starter,
        "pipeline-new-start-starter",
        "pipeline_new.start.starter",
      ),
      (starter_lines, None),
    ),
    option(
      ctx,
      start,
      (Choice::Copy, "pipeline-new-start-copy", "pipeline_new.start.copy"),
      (copy_lines(ctx, start, chosen), Some(copy_select(ctx, start))),
    ),
    option(
      ctx,
      start,
      (Choice::Empty, "pipeline-new-start-empty", "pipeline_new.start.empty"),
      (
        vec![(
          ctx.t("pipeline_new.start.emptySub").to_string(),
          PaletteColor::TextSecondary,
        )],
        None,
      ),
    ),
  ];
  rows::section(
    "pipeline-new-start",
    &ctx.t("pipeline_new.start.title"),
    vec![lead.into()],
    rows,
  )
  .into()
}

/// What *Copy a pipeline* says: what becomes v1, and once a pipeline is chosen and the row is, its steps.
fn copy_lines(ctx: &Ctx, start: &Start<'_>, chosen: Choice) -> Vec<(String, PaletteColor)> {
  let source = start.source.filter(|_| chosen == Choice::Copy);
  let Some(body) = source.and_then(|entry| entry.body.as_ref()) else {
    return vec![(
      ctx.t("pipeline_new.start.copySub").to_string(),
      PaletteColor::TextSecondary,
    )];
  };
  vec![
    (
      ctx.t("pipeline_new.start.copyChosen").to_string(),
      PaletteColor::TextSecondary,
    ),
    (
      ctx
        .t_args(
          "pipeline_new.start.copySteps",
          [("steps", body.steps().len().to_string()), ("chain", chain(ctx, body))],
        )
        .to_string(),
      PaletteColor::TextMuted,
    ),
  ]
}

/// The pipelines to copy, as *name · version*.
fn copy_select(ctx: &Ctx, start: &Start<'_>) -> Element {
  let options = start
    .copies
    .iter()
    .filter_map(|entry| {
      let version = entry.number?;
      Some(SelectOption::new(
        entry.id.to_string(),
        ctx.t_args(
          "pipeline_new.start.copyOption",
          [("name", entry.name.to_string()), ("version", version.to_string())],
        ),
      ))
    })
    .collect();
  let select = SelectField::new(
    "pipeline-new-copy",
    ctx.t("pipeline_new.start.copy"),
    start.copy.clone(),
    options,
  )
  .placeholder(ctx.t("pipeline_new.start.choosePipeline"))
  .width(Dimension::Px(COPY_WIDTH));
  if start.locked { select.locked() } else { select }.render(ctx)
}

/// A row: its radio, title and lines, which choose it, and at its end `end`.
fn option(
  ctx: &Ctx,
  start: &Start<'_>,
  (choice, id, title): (Choice, &str, &str),
  (lines, end): (Vec<(String, PaletteColor)>, Option<Element>),
) -> Row {
  let on = start.choice.get() == choice;
  let words = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(&ctx.t(title))
        .id(format!("{id}-title"))
        .variant(ExtraTypography::OptionTitle.role()),
    )
    .with_children(lines.into_iter().enumerate().map(|(index, (line, color))| {
      Text::new(&line)
        .id(format!("{id}-line-{index}"))
        .variant(HubText::RowLine.role())
        .color(color)
    }));
  let state = if on { "chosen" } else { "not chosen" };
  // While *Create v1* stores the pipeline nothing is chosen: the row is not a button then.
  let pick: Element = if start.locked {
    Row::new()
      .id(format!("{id}-pick"))
      .describe("state", state)
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Start)
      .child(radio_mark(on))
      .child(words)
      .into()
  } else {
    let signal = start.choice.clone();
    Press::run(move || signal.set(choice))
      .button()
      .id(format!("{id}-pick"))
      .describe("state", state)
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Start)
      .child(radio_mark(on))
      .child(words)
      .into()
  };
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(pick)
    .with_children(end)
}

/// The steps of `pipeline` by stage, those that run side by side joined (`Research → Plan → … → Run checks ∥ Review`).
pub(super) fn chain(ctx: &Ctx, pipeline: &Pipeline) -> String {
  let (then, parallel) = (ctx.t("pipeline_new.then"), ctx.t("pipeline_new.parallel"));
  levels(pipeline.steps())
    .iter()
    .map(|level| {
      level
        .iter()
        .map(|index| pipeline.steps()[*index].name.as_str().to_owned())
        .collect::<Vec<_>>()
        .join(&parallel)
    })
    .collect::<Vec<_>>()
    .join(&then)
}
