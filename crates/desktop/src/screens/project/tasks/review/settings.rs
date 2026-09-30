//! Execution mode and limits from W1Dq6/IyJ5a.
use super::*;

impl SchedulingReview {
  pub(super) fn settings(&self, ctx: &Ctx, props: &ReviewProps, locked: bool) -> Element {
    let fields = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(Text::new(&ctx.t("tasks.bulk.executionMode")).variant(ExtraTypography::SectionHeading.role()))
      .child(self.mode_options(ctx, locked))
      .child(self.scheduling_field(ctx, locked))
      .child(Text::new(&self.capacity_note(ctx, props)).variant(TypographyStyle::Caption))
      .child(field(
        &ctx.t("tasks.bulk.failure"),
        self.fixed_select(ctx, "tasks-review-failure", "tasks.bulk.pauseRemaining", locked),
        FieldNote::None,
      ))
      .child(self.limit_fields(ctx, locked))
      .child(self.pipeline_readback(ctx, props))
      .child(self.approval_notice(ctx, props))
      .child(Text::new(&ctx.t("tasks.bulk.branchNote")).variant(TypographyStyle::Caption));
    Column::new()
      .id("tasks-review-settings")
      .width(Dimension::Px(REVIEW_RAIL_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding_left(SpacingSize::Section)
      .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(
        ScrollVertical::new(fields)
          .width(Dimension::Pct(100.0))
          .height(Dimension::Pct(100.0))
          .scrollbar(gutter_scrollbar()),
      )
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
      .spacing(ExtraSpacing::Stack.role())
      .with_children(modes.into_iter().map(|(mode, id, title, detail)| {
        let selected = self.mode.get() == mode;
        let state = self.mode.clone();
        let card = OptionCard::new(id, ctx.t(title), ctx.t(detail), selected).scheduling_review();
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
        self.fixed_select(ctx, "tasks-review-gate", "tasks.bulk.reviewGate", locked),
        FieldNote::None,
      );
    }
    let choices = (1..=self.order.get().0.len().max(1) as u32)
      .filter(|n| !locked || *n == self.concurrent.get())
      .map(|n| (n, ctx.t_args("tasks.bulk.concurrentCount", [("count", n.to_string())])));
    let control = Element::from(
      Select::new(self.concurrent.clone())
        .options(choices)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(REVIEW_FIELD_HEIGHT))
        .focusable(!locked)
        .style(select_style(ctx, REVIEW_FIELD_HEIGHT)),
    )
    .id("tasks-review-concurrent");
    field(&ctx.t("tasks.bulk.concurrent"), control, FieldNote::None)
  }

  fn limit_fields(&self, ctx: &Ctx, locked: bool) -> Element {
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Md)
          .child(Column::new().flex(1.0).child(self.token_field(
            ctx,
            "tasks-review-cap",
            ("tasks.bulk.groupCap", "tasks.bulk.totalTokens"),
            self.cap.clone(),
            locked,
          )))
          .child(Column::new().flex(1.0).child(self.token_field(
            ctx,
            "tasks-review-budget",
            ("tasks.bulk.perTask", "tasks.bulk.eachTokens"),
            self.budget.clone(),
            locked,
          ))),
      )
      .child(self.time_readback(ctx, locked))
      .into()
  }

  /// The scheduler currently supports one gate and one failure policy. Keep the select
  /// composition, with only truthful choices rather than offering unsupported behavior.
  fn fixed_select(&self, ctx: &Ctx, id: &'static str, key: &str, locked: bool) -> Element {
    Element::from(
      Select::new(self.fixed_policy.clone())
        .options([(0, ctx.t(key))])
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(REVIEW_FIELD_HEIGHT))
        .focusable(!locked)
        .style(select_style(ctx, REVIEW_FIELD_HEIGHT)),
    )
    .id(id)
  }
}
