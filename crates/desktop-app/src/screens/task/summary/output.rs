//! What the run made (`Output` `dJKKA` in `01 · Summary`): *Output* with its count, then a row per step's report on
//! `surface_raised`, radius 6, 8 over and under and 10 at its sides, 8 apart (`Run summary` `KunXT`): a 14
//! `text_secondary` glyph, its title in 13/500, *after 05 Review · 15:34* in 12 `text_muted`, and a 13 `arrow-up-right`
//! at its end; the row opens the Conversation, where the step's report is. The report the run delivers is the *Run
//! summary*, the latest first. Runs keep no other artifacts yet: with no report the section says so.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::run::StepOutput;
use orchester_workspace::{display, tasks::detail::RunView};

use super::section_head;
use crate::{
  navigation::Navigation,
  routes::{Route, TaskTab, TaskView},
  screens::task::TaskContext,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
  },
};

pub fn section(ctx: &mut Ctx, task: &TaskContext, run: &RunView) -> Element {
  let outputs = reports(run);
  let head = section_head(ctx, "task-output-head", "summary.output.title", Some(outputs.len()));
  let rows: Vec<Element> = if outputs.is_empty() {
    vec![
      Text::new(&ctx.t("summary.output.none"))
        .id("task-output-none")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Pct(100.0))
        .into(),
    ]
  } else {
    let delivered = task
      .detail
      .result
      .as_ref()
      .and_then(|result| result.report.as_ref())
      .and_then(|report| report.step.clone());
    outputs
      .iter()
      .enumerate()
      .map(|(index, output)| {
        row(
          ctx,
          task,
          run,
          (index, output),
          delivered.as_ref() == Some(&output.step),
        )
      })
      .collect()
  };
  Column::new()
    .id("task-output")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(head)
    .with_children(rows)
    .into()
}

/// Each step's latest report that says something, the latest first.
fn reports(run: &RunView) -> Vec<&StepOutput> {
  let mut latest: Vec<&StepOutput> = Vec::new();
  for output in run.outputs.iter().filter(|output| !output.text.trim().is_empty()) {
    match latest.iter_mut().find(|kept| kept.step == output.step) {
      Some(kept) if kept.created_at < output.created_at => *kept = output,
      Some(_) => {}
      None => latest.push(output),
    }
  }
  latest.sort_by_key(|output| std::cmp::Reverse(output.created_at));
  latest
}

/// A report's row: the run's summary when it is the report the run delivers (`delivered`), else the step's report.
fn row(
  ctx: &mut Ctx,
  task: &TaskContext,
  run: &RunView,
  (index, output): (usize, &StepOutput),
  delivered: bool,
) -> Element {
  let step = run.steps.iter().find(|step| step.key == output.step);
  let name: Arc<str> = step.map_or_else(|| Arc::from(output.step.as_str()), |step| step.name.clone());
  let (title, glyph) = if delivered {
    (ctx.t("summary.output.runSummary"), Icon::FileText)
  } else {
    let glyph = if step.is_some_and(|step| step.tool_step) {
      Icon::Terminal
    } else {
      Icon::FileText
    };
    (ctx.t_args("summary.output.report", [("step", &*name)]), glyph)
  };
  let label = step.map_or(name, |step| {
    ctx.t_args(
      "summary.output.step",
      [
        ("number", format!("{:02}", step.number)),
        ("step", step.name.to_string()),
      ],
    )
  });
  let when = ctx.t_args(
    "summary.output.after",
    [("step", &*label), ("time", display::clock(output.created_at).as_str())],
  );
  let navigation = Navigation::of(ctx);
  let route = Route::Project(
    task.project.open.project.id(),
    task.page(TaskView::of(TaskTab::Conversation)),
  );
  opening(navigation.to(route))
    .id(format!("task-output-{index}"))
    .describe("a11y_name", title.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&title)
        .id(format!("task-output-{index}-title"))
        .variant(ExtraTypography::Strong.role())
        .nowrap(),
    )
    .child(
      Text::new(&when)
        .id(format!("task-output-{index}-when"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap(),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Base, PaletteColor::TextMuted))
    .into()
}
