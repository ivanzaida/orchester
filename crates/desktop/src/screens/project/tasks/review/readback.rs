//! Derived scheduling summaries and compact properties; time editing is an undrawn state.
use orchester_core::tokens::compact_tokens;

use super::*;
use crate::ui::{
  pressable::{Ring, keyboard_stop},
  property::{property, wrapping_top_property},
  unit_input::UnitInput,
};

impl SchedulingReview {
  pub(super) fn summary(&self, ctx: &Ctx) -> Arc<str> {
    ctx.t_args(
      if self.mode.get() == Mode::Parallel {
        "tasks.bulk.summaryParallel"
      } else {
        "tasks.bulk.summarySequential"
      },
      [
        ("count", self.order.get().tasks.len().to_string()),
        ("concurrent", self.concurrent.get().to_string()),
      ],
    )
  }

  pub(super) fn capacity_note(&self, ctx: &Ctx, props: &ReviewProps) -> Arc<str> {
    if self.mode.get() == Mode::Sequential {
      return ctx.t("tasks.bulk.sequentialCapacity");
    }
    match props.agent_slots {
      Some(slots) => ctx.t_args("tasks.bulk.capacitySlots", [("slots", slots.to_string())]),
      None => ctx.t("tasks.bulk.capacityNote"),
    }
  }

  pub(super) fn token_field(
    &self,
    ctx: &Ctx,
    id: &'static str,
    keys: (&str, &str),
    value: Signal<String>,
    locked: bool,
  ) -> Element {
    let separator = ctx.t("number.groupSeparator");
    let parsed = read_budget(&value.get(), &separator, budget::unbounded());
    let input = UnitInput::new(id, value, ctx.t("tokens.unit"), self.focused.clone())
      .tokens(separator)
      .read_only(locked)
      .invalid(parsed.is_err())
      .size(Dimension::Pct(100.0), Dimension::Px(REVIEW_FIELD_HEIGHT))
      .render(ctx);
    let note = parsed.into_iter().map(|budget| {
      Element::from(
        Text::new(&ctx.t_args(keys.1, [("count", compact_tokens(budget.get()))]))
          .variant(ExtraTypography::Hint.role())
          .color(lurq::app::theme::PaletteColor::TextMuted),
      )
      .id(format!("{id}-readback"))
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(field(&ctx.t(keys.0), input, FieldNote::None))
      .with_children(note)
      .into()
  }

  pub(super) fn time_readback(&self, ctx: &Ctx, locked: bool) -> Element {
    if self.editing_time.get() && !locked {
      let editing = self.editing_time.clone();
      return Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(
          Column::new().flex(1.0).child(
            UnitInput::new(
              "tasks-review-time",
              self.minutes.clone(),
              ctx.t("tasks.new.minutes"),
              self.focused.clone(),
            )
            .size(Dimension::Pct(100.0), Dimension::Px(REVIEW_FIELD_HEIGHT))
            .render(ctx),
          ),
        )
        .child(
          ActionButton::new(ctx.t("tasks.bulk.finishTime"))
            .id("tasks-review-time-done")
            .in_form()
            .on_click(move || editing.set(false))
            .render(ctx),
        )
        .into();
    }
    let value = ctx.t_args("tasks.bulk.timeValue", [("minutes", self.minutes.get())]);
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(crate::theme::TASK_REVIEW_PROPERTY_HEIGHT))
      .align_items(lurq::layout::Alignment::Center)
      .child(property(&ctx.t("tasks.bulk.timeLimit"), &value));
    let editing = self.editing_time.clone();
    let button = surface()
      .id("tasks-review-time-readback")
      .width(Dimension::Pct(100.0))
      .child(row);
    if locked {
      button.into()
    } else {
      keyboard_stop(button.on_click(move |_| editing.set(true)), Ring::Inset).into()
    }
  }

  pub(super) fn pipeline_readback(&self, ctx: &Ctx, props: &ReviewProps) -> Element {
    let mut versions = std::collections::BTreeSet::new();
    if let Some(tasks) = props.tasks.ready() {
      for task in tasks
        .tasks
        .iter()
        .filter(|task| self.order.get().tasks.contains(&task.id()))
      {
        if let Some(pipeline) = tasks.pipelines.iter().find(|p| Some(p.id) == task.details().pipeline) {
          let version = task.pipeline_version().unwrap_or(pipeline.version);
          versions.insert(
            ctx
              .t_args(
                "tasks.bulk.pipelineValue",
                [
                  ("pipeline", pipeline.name.to_string()),
                  ("version", version.to_string()),
                ],
              )
              .to_string(),
          );
        }
      }
    }
    let value = versions
      .into_iter()
      .collect::<Vec<_>>()
      .join(&ctx.t("tasks.bulk.pipelineSeparator"));
    Row::new()
      .id("tasks-review-pipeline")
      .width(Dimension::Pct(100.0))
      .min_height(Dimension::Px(crate::theme::TASK_REVIEW_PROPERTY_HEIGHT))
      .child(wrapping_top_property(&ctx.t("tasks.bulk.pipelineVersion"), &value))
      .into()
  }

  pub(super) fn approval_notice(&self, ctx: &Ctx, props: &ReviewProps) -> Element {
    let order = self.order.get().tasks;
    let all_gated = !order.is_empty()
      && props.tasks.ready().is_some_and(|tasks| {
        order.iter().all(|id| {
          tasks
            .tasks
            .iter()
            .find(|task| task.id() == *id)
            .and_then(|task| {
              tasks
                .pipelines
                .iter()
                .find(|pipeline| Some(pipeline.id) == task.details().pipeline)
            })
            .is_some_and(|pipeline| pipeline.steps.iter().any(|step| step.approval_gate))
        })
      });
    let heading = ctx.t(if all_gated {
      "tasks.bulk.planApprovalHeading"
    } else {
      "tasks.bulk.approvalHeading"
    });
    let note = if all_gated {
      ctx.t_args(
        if self.mode.get() == Mode::Parallel {
          "tasks.bulk.parallelApprovalNote"
        } else {
          "tasks.bulk.sequentialApprovalNote"
        },
        [("concurrent", self.concurrent.get().to_string())],
      )
    } else {
      ctx.t("tasks.bulk.approvalNote")
    };
    crate::ui::notice::Notice::new(Icon::Hand, heading, note).into()
  }
}
