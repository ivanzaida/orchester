//! Execution mode and limits from W1Dq6/IyJ5a.
use super::*;

impl SchedulingReview {
  pub(super) fn settings(&self, ctx: &Ctx, locked: bool) -> Element {
    let fields = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(Text::new(&ctx.t("tasks.bulk.executionMode")).variant(ExtraTypography::SectionHeading.role()))
      .child(self.mode_options(ctx, locked))
      .child(self.scheduling_field(ctx, locked))
      .child(Text::new(&ctx.t("tasks.bulk.capacityNote")).variant(TypographyStyle::Caption))
      .child(field(
        &ctx.t("tasks.bulk.failure"),
        Text::new(&ctx.t("tasks.bulk.pauseRemaining"))
          .variant(TypographyStyle::Body)
          .into(),
        FieldNote::None,
      ))
      .child(self.limit_fields(ctx, locked))
      .child(crate::ui::notice::Notice::new(
        Icon::Hand,
        ctx.t("tasks.bulk.approvalHeading"),
        ctx.t("tasks.bulk.approvalNote"),
      ))
      .child(Text::new(&ctx.t("tasks.bulk.branchNote")).variant(TypographyStyle::Caption));
    ScrollVertical::new(fields)
      .width(Dimension::Px(REVIEW_RAIL_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding_left(SpacingSize::Section)
      .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .scrollbar(gutter_scrollbar())
      .into()
  }

  fn mode_options(&self, ctx: &Ctx, locked: bool) -> Row {
    let modes = [
      (
        Mode::Parallel,
        "tasks-review-parallel",
        "tasks.bulk.parallel",
        "tasks.bulk.parallelDetail",
      ),
      (
        Mode::Sequential,
        "tasks-review-sequential",
        "tasks.bulk.sequential",
        "tasks.bulk.sequentialDetail",
      ),
    ];
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(modes.into_iter().map(|(mode, id, title, detail)| {
        let selected = self.mode.get() == mode;
        let state = self.mode.clone();
        let card = OptionCard::new(id, ctx.t(title), ctx.t(detail), selected);
        let card = if locked {
          card.unavailable(None)
        } else {
          card.on_choose(move || state.set(mode))
        };
        Column::new().flex(1.0).child(card)
      }))
  }

  fn scheduling_field(&self, ctx: &Ctx, locked: bool) -> Element {
    if self.mode.get() == Mode::Sequential {
      return field(
        &ctx.t("tasks.bulk.nextWhen"),
        Text::new(&ctx.t("tasks.bulk.reviewGate"))
          .variant(TypographyStyle::Body)
          .into(),
        FieldNote::None,
      );
    }
    let control: Element = if locked {
      Text::new(&ctx.t_args(
        "tasks.bulk.concurrentCount",
        [("count", self.concurrent.get().to_string())],
      ))
      .variant(TypographyStyle::Body)
      .into()
    } else {
      let choices = (1..=self.order.get().0.len().max(1) as u32)
        .map(|n| (n, ctx.t_args("tasks.bulk.concurrentCount", [("count", n.to_string())])));
      Element::from(
        Select::new(self.concurrent.clone())
          .options(choices)
          .width(Dimension::Pct(100.0))
          .height(Dimension::Px(REVIEW_FIELD_HEIGHT))
          .style(select_style(ctx, REVIEW_FIELD_HEIGHT)),
      )
      .id("tasks-review-concurrent")
    };
    field(&ctx.t("tasks.bulk.concurrent"), control, FieldNote::None)
  }

  fn limit_fields(&self, ctx: &Ctx, locked: bool) -> Element {
    let input = |id, label, value: Signal<String>| {
      field(
        &ctx.t(label),
        text_input(ctx, id, value, false, TypographyStyle::Body)
          .on_input(move |event: lurq::app::events::TextInputEvent| {
            if locked {
              event.prevent_default();
            }
          })
          .into(),
        FieldNote::None,
      )
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Md)
          .child(
            Column::new()
              .flex(1.0)
              .child(input("tasks-review-cap", "tasks.bulk.groupCap", self.cap.clone())),
          )
          .child(Column::new().flex(1.0).child(input(
            "tasks-review-budget",
            "tasks.bulk.perTask",
            self.budget.clone(),
          ))),
      )
      .child(input("tasks-review-time", "tasks.bulk.timeLimit", self.minutes.clone()))
      .into()
  }
}
