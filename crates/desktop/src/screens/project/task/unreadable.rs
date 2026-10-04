//! A task whose latest run cannot be read (02.R `SUuwV`): Cancel run shown unavailable in the
//! topbar (`asPAJ`), and as the Summary the emphasized notice (`J9tqH`), why the run's actions are
//! unavailable (`AZI67`) and the task's runs (*Runs of this task*, which replaced `Msc86`). An earlier run that cannot
//! be read shows the same notice below the heading of the latest run's page (02.R3 `cP8oh`).

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, ScrollVertical},
  layout::layout_kind::ScrollState,
  node::{Element, dimension::Dimension},
};

use super::super::page;
use crate::{
  tasks::unreadable::Unreadable,
  theme::{ExtraSpacing, gutter_scrollbar},
  ui::{
    button::ActionButton,
    icon::Icon,
    message_text,
    notice::{Notice, lock_line},
  },
};

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

/// An earlier run of the task that cannot be read, below the page heading (`hD7nI` in 02.R3 `cP8oh`).
pub(super) fn earlier_notice(ctx: &Ctx, run: &Unreadable) -> Element {
  Column::new()
    .id(format!("task-unreadable-{}", run.id))
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(SpacingSize::Md)
    .child(notice(ctx, run))
    .into()
}

/// The Summary of a task whose latest run `run` cannot be read, with `runs`, *Runs of this task*
/// (`w6AfG`, ORC-63); it scrolls with the scroll gutter when the runs outgrow the window.
pub(super) fn summary(ctx: &Ctx, (run, scroll): (&Unreadable, &ScrollState), runs: Option<Element>) -> Element {
  let actions = lock_line("task-unreadable-actions", &message_text(ctx, &run.actions()));
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
    .with_children(runs);
  ScrollVertical::new(content)
    .id("task-unreadable")
    .with_scroll_state(scroll.clone())
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .scrollbar(gutter_scrollbar())
    .into()
}
