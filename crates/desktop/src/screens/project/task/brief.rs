//! What a task asks for, on its Summary (ORC-12 in the design doc; `tqn5P`, `uv4PY`): the brief
//! and the acceptance criteria as a numbered list, after the run's steps when it has a run. A task
//! without a run shows its pipeline and limits in *This task* (`FLGqL`; nothing chosen `h3IIs`),
//! and a draft that can still be edited has *Edit* in the topbar.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  task::{TaskDetails, TaskLimits},
  tokens::compact_tokens,
};

use super::{super::page, summary::this_run_panel};
use crate::{
  budget::whole_minutes,
  navigation::route::Page,
  screens::project::ProjectNav,
  tasks::{data::PipelineChoice, detail::TaskDetail},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::Icon,
    property::{ValueStyle, property, styled_property},
  },
};

/// Width of a criterion's number (`a88TT`).
const NUMBER_WIDTH: f32 = 14.0;

/// *Brief* and *Acceptance criteria* (`OkzPJ`); a part the task does not have yet is left out, and
/// so is the section when it has neither.
pub(super) fn brief_section(ctx: &Ctx, details: &TaskDetails) -> Option<Element> {
  let brief = (!details.brief.trim().is_empty()).then(|| {
    Column::new()
      .id("task-brief")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .child(heading(ctx, "runs.summary.brief"))
      .child(
        Text::new(&details.brief)
          .variant(ExtraTypography::Brief.role())
          .width(Dimension::Pct(100.0))
          .selectable(true),
      )
  });
  let criteria = (!details.acceptance_criteria.is_empty()).then(|| {
    Column::new()
      .id("task-criteria")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .child(heading(ctx, "runs.summary.criteria"))
      .with_children(
        (1..)
          .zip(&details.acceptance_criteria)
          .map(|(number, text)| criterion(number, text)),
      )
  });
  if brief.is_none() && criteria.is_none() {
    return None;
  }
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .with_children(brief)
      .with_children(criteria)
      .into(),
  )
}

fn heading(ctx: &Ctx, key: &str) -> Text {
  Text::new(&ctx.t(key)).variant(ExtraTypography::SectionHeading.role())
}

/// One numbered criterion (`L9DuW`).
fn criterion(number: usize, text: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Start)
    .child(
      Text::new(&number.to_string())
        .variant(ExtraTypography::Criterion.role())
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(NUMBER_WIDTH)),
    )
    .child(
      Text::new(text)
        .variant(ExtraTypography::Criterion.role())
        .flex(1.0)
        .selectable(true),
    )
}

/// *This task* (`FLGqL`): the pipeline a run would start with, at its latest published version, and
/// the limits; *No pipeline chosen* and *No limits set* while they are not (`h3IIs`).
pub(super) fn this_task(
  ctx: &Ctx,
  detail: &TaskDetail,
  pipelines: Option<&[PipelineChoice]>,
  scroll: &ScrollState,
) -> Element {
  let details = detail.task.details();
  let pipeline = match details.pipeline {
    None => styled_property(
      &ctx.t("runs.summary.pipeline"),
      &ctx.t("runs.summary.noPipeline"),
      ValueStyle::Unset,
    ),
    Some(chosen) => {
      let choice = pipelines.and_then(|choices| choices.iter().find(|choice| choice.id == chosen));
      let value = choice.map_or_else(
        || ctx.t("tasks.pipeline.unavailable"),
        |choice| {
          ctx.t_args(
            "runs.summary.pipelineValue",
            [
              ("name", &*choice.name),
              ("version", choice.version.to_string().as_str()),
            ],
          )
        },
      );
      property(&ctx.t("runs.summary.pipeline"), &value)
    }
  };
  let rows = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Step.role())
    .child(Text::new(&ctx.t("runs.summary.thisTask")).variant(ExtraTypography::CardTitle.role()))
    .child(pipeline)
    .with_children(limit_rows(ctx, details.limits));
  this_run_panel(rows, scroll, &ctx.t("runs.summary.taskNote"))
}

fn limit_rows(ctx: &Ctx, limits: Option<TaskLimits>) -> Vec<Element> {
  let Some(limits) = limits else {
    return vec![styled_property(
      &ctx.t("runs.summary.limits"),
      &ctx.t("runs.summary.noLimits"),
      ValueStyle::Unset,
    )];
  };
  let budget = ctx.t_args(
    "runs.tokens",
    [("count", compact_tokens(limits.budget().get()).as_str())],
  );
  let minutes = whole_minutes(limits.max_duration()).to_string();
  let time_limit = ctx.t_args("runs.summary.timeLimitValue", [("minutes", minutes.as_str())]);
  vec![
    property(&ctx.t("runs.summary.budget"), &budget),
    property(&ctx.t("runs.summary.timeLimit"), &time_limit),
  ]
}

/// *Edit* (`IyZyB`), for a draft that can still be edited: opens it in the New task form.
pub(super) fn edit_button(ctx: &Ctx, nav: &ProjectNav, detail: &TaskDetail) -> Option<Element> {
  if !detail.editable() {
    return None;
  }
  let (nav, task) = (nav.clone(), detail.task.id());
  Some(
    ActionButton::new(ctx.t("tasks.edit.action"))
      .icon(Icon::Pencil)
      .id("task-edit")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
      .on_click(move || nav.show(Page::EditTask(task)))
      .render(ctx),
  )
}
