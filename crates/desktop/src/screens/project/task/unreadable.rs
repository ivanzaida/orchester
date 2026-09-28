//! A task whose latest run cannot be read (02.R `SUuwV`): Cancel run shown unavailable in the
//! topbar (`asPAJ`), and as the Summary the emphasized notice (`J9tqH`), why the run's actions are
//! unavailable (`AZI67`) and the task's other runs (`Msc86`). An earlier run that cannot be read
//! shows the same notice below the heading of the latest run's page (not drawn yet).

use std::sync::Arc;

use chrono::Datelike;
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::time::Timestamp;

use super::super::{page, task_rows::row_status_label};
use crate::{
  display::{self, MONTH_KEYS},
  tasks::unreadable::{EarlierRun, Unreadable},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    message_text,
    notice::Notice,
  },
};

/// Columns and row height of the other runs (`p4BSb`, `F5bXbW`, `GHwIe`).
const RUN_WIDTH: f32 = 72.0;
const STATUS_WIDTH: f32 = 128.0;
const ROW_HEIGHT: f32 = 46.0;

/// The run controls of a run that cannot be read (`asPAJ`): Cancel run, unavailable.
pub(super) fn controls(ctx: &Ctx) -> Vec<Element> {
  vec![
    ActionButton::new(ctx.t("runs.page.cancel"))
      .icon(Icon::Square)
      .id("task-cancel-run")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
      .render(ctx),
  ]
}

/// "Run #131 could not be read", why, and that the database file is kept unchanged (`J9tqH`).
fn notice(ctx: &Ctx, run: &Unreadable) -> Notice {
  Notice::new(
    Icon::AlertCircle,
    message_text(ctx, &run.title()),
    message_text(ctx, &run.detail),
  )
  .emphasized()
}

/// An earlier run of the task that cannot be read, below the page heading (not drawn yet).
pub(super) fn earlier_notice(ctx: &Ctx, run: &Unreadable) -> Element {
  Column::new()
    .id(format!("task-unreadable-{}", run.id))
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(SpacingSize::Md)
    .child(notice(ctx, run))
    .into()
}

/// The Summary of a task whose latest run `run` cannot be read, with the task's `earlier` runs,
/// newest first (`w6AfG`); it scrolls with the scroll gutter when the runs outgrow the window.
pub(super) fn summary(ctx: &Ctx, (run, scroll): (&Unreadable, &ScrollState), earlier: &[EarlierRun]) -> Element {
  let actions = Row::new()
    .id("task-unreadable-actions")
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextSecondary))
    .child(
      Text::new(&message_text(ctx, &run.actions()))
        .variant(TypographyStyle::Caption)
        .flex(1.0),
    );
  let content = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Section)
    .padding_left(SpacingSize::Section)
    .padding_right(ExtraSpacing::BesideGutter.role())
    .padding_bottom(SpacingSize::Section)
    .spacing(SpacingSize::Lg)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Stack.role())
        .child(notice(ctx, run).id("task-unreadable-notice"))
        .child(actions),
    )
    .child(other_runs(ctx, earlier));
  ScrollVertical::new(content)
    .id("task-unreadable")
    .with_scroll_state(scroll.clone())
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .scrollbar(gutter_scrollbar())
    .into()
}

/// Other runs of this task (`Msc86`), newest first.
fn other_runs(ctx: &Ctx, earlier: &[EarlierRun]) -> Element {
  let explanation = if earlier.is_empty() {
    "runs.unreadable.otherRuns.none"
  } else {
    "runs.unreadable.otherRuns.detail"
  };
  Column::new()
    .id("task-other-runs")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("runs.unreadable.otherRuns.title")).variant(ExtraTypography::SectionHeading.role()))
    .child(Text::new(&ctx.t(explanation)).variant(TypographyStyle::Caption))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .with_children(earlier.iter().map(|run| other_run(ctx, run))),
    )
    .into()
}

/// One of the other runs (`GHwIe`): its number, status, what it means and when it last changed.
fn other_run(ctx: &Ctx, run: &EarlierRun) -> Element {
  let number = run.number.map_or_else(
    || ctx.t("runs.unreadable.otherRuns.runUnnumbered"),
    |number| {
      ctx.t_args(
        "runs.unreadable.otherRuns.run",
        [("run", number.get().to_string().as_str())],
      )
    },
  );
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(&number)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .width(Dimension::Px(RUN_WIDTH)),
    )
    .child(
      Row::new()
        .width(Dimension::Px(STATUS_WIDTH))
        .child(row_status_label(ctx, run.phase)),
    )
    .child(
      Text::new(&message_text(ctx, &run.detail))
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0),
    )
    .with_children(run.at.and_then(|at| day_and_clock(ctx, at)).map(|at| {
      Text::new(&at)
        .variant(ExtraTypography::MonoValue.role())
        .color(PaletteColor::TextSecondary)
        .nowrap()
    }))
    .into()
}

/// Local date and time of `at`, e.g. "Sep 26 · 11:04"; `None` for an instant chrono cannot place,
/// which no stored change has.
fn day_and_clock(ctx: &Ctx, at: Timestamp) -> Option<Arc<str>> {
  let local = chrono::DateTime::from_timestamp_millis(at.unix_millis())?.with_timezone(&chrono::Local);
  let month = ctx.t(MONTH_KEYS[local.month0() as usize]);
  let day = local.day().to_string();
  let time = display::clock(at);
  Some(ctx.t_args(
    "time.dayAndClock",
    [("month", &*month), ("day", day.as_str()), ("time", time.as_str())],
  ))
}
