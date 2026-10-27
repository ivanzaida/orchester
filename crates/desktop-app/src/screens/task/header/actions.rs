//! The title row (`Title row` `QIedd` in `j8DakG`): the task's title in 20/600 as wide as the row leaves it, then the
//! one primary action the task's state calls for (`header::primary`) and *More actions* (`header::more`), 12 apart.
//! A primary action that cannot be used now says why in full in its tooltip and in a few words under it, except on
//! a run that cannot be read, whose Summary says it already.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{HeaderProps, controls::Control};
use crate::{
  screens::task::sharing,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    chip::{BadgeTone, badge},
    tooltip::{Tooltip, Truncated},
  },
};

/// The row with `primary`, the header's one primary action, if any, and `more`, its *More actions*, last.
pub fn title_row(ctx: &mut Ctx, props: &HeaderProps, primary: Option<Control>, more: Element) -> Element {
  let title = Truncated::new(
    "task-title",
    props.task.detail.task.details().title.as_str().to_owned(),
    TypographyStyle::Heading,
  )
  .render(ctx);
  let title = Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .child(title),
    )
    .with_children(after_title(ctx, props));
  Row::new()
    .id("task-title-row")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(title)
    .with_children(primary.map(|primary| primary_action(ctx, props, &primary)))
    .child(Row::new().id("task-actions").child(more))
    .into()
}

/// After the title: *Not shared* for a task kept here, *was KON-12* for one that went up with another key.
fn after_title(ctx: &Ctx, props: &HeaderProps) -> Option<Element> {
  if props.task.detail.not_shared() {
    return Some(
      badge(
        "task-not-shared-badge",
        BadgeTone::Neutral,
        &ctx.t("task.notShared.badge"),
      )
      .into(),
    );
  }
  sharing::former_key(&props.task).map(|former| {
    Text::new(&ctx.t_args("task.formerKey", [("key", former)]))
      .id("task-former-key")
      .variant(ExtraTypography::MonoKey.role())
      .color(PaletteColor::TextMuted)
      .nowrap()
      .into()
  })
}

/// The primary action, and when it cannot be used now why in a few words under it, its whole reason in its tooltip.
fn primary_action(ctx: &mut Ctx, props: &HeaderProps, primary: &Control) -> Element {
  let button: Element = match primary.reason() {
    Some(reason) => Tooltip::new("task-action-tooltip-0", reason.full.clone()).wrap(ctx, primary.button()),
    None => primary.button().into(),
  };
  // A run that cannot be read says why on its Summary; the line would say it twice.
  let line = primary
    .reason()
    .filter(|_| props.task.detail.latest_unreadable.is_none())
    .map(|reason| {
      Text::new(&reason.short)
        .id("task-action-reasons")
        .variant(TypographyStyle::Caption)
        .nowrap()
        .describe("role", "unavailable-reason")
    });
  Column::new()
    .id("task-primary")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::End)
    .child(button)
    .with_children(line)
    .into()
}
